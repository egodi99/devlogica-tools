//! Disegno degli indicatori (CPU, RAM, orologio, logo) sopra lo sfondo animato, con egui,
//! nella stessa immagine e nello stesso fotogramma dello shader. Gli shader non cambiano.

use crate::config::{IndicatorStyle, Indicators, Side};
use crate::indicators::{self, Block, DockPlace, Item, EDGE_MARGIN};
use crate::render::Gpu;
use crate::sysstats::Stats;
use egui::{text::LayoutJob, Color32, FontId, Galley, Pos2, Rect, Stroke, TextFormat, Vec2};
use std::sync::Arc;

/// Tutto quello che serve per disegnare un fotogramma degli indicatori.
pub struct Frame<'a> {
    pub ind: &'a Indicators,
    pub stats: Stats,
    pub hm: (u32, u32),
    pub dock: DockPlace,
    /// 0..1: dissolvenza di comparsa insieme al logo grande dello sfondo.
    pub opacity: f32,
}

pub struct Overlay {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
    logo: egui::TextureHandle,
}

const LOGO_H: f32 = 18.0;
const PILL_PAD: Vec2 = Vec2::new(10.0, 5.0);

/// Logo ridotto una volta sola (egui-wgpu non crea mipmap): tre volte l'altezza disegnata,
/// così resta nitido sia sugli schermi Retina sia su quelli normali.
fn logo_image() -> image::RgbaImage {
    let img = image::load_from_memory(include_bytes!("../assets/logo.png")).expect("logo").to_rgba8();
    let h = (LOGO_H * 3.0) as u32;
    let w = (img.width() as f32 * h as f32 / img.height() as f32).round() as u32;
    image::imageops::resize(&img, w, h, image::imageops::FilterType::Lanczos3)
}

/// Un blocco già misurato, pronto da disegnare.
enum Piece {
    Logo,
    Text(Arc<Galley>),
    Stack { clock: Arc<Galley>, rows: Vec<Arc<Galley>> },
}

impl Overlay {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        let img = logo_image();
        let size = [img.width() as usize, img.height() as usize];
        let color = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
        let logo = ctx.load_texture("logo", color, egui::TextureOptions::LINEAR);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, format, egui_wgpu::RendererOptions::default());
        Self { ctx, renderer, logo }
    }

    /// Disegna sopra il contenuto già presente in `view`. `size_px` è la dimensione della
    /// superficie, `ppp` i pixel per punto della superficie.
    pub fn draw(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size_px: [u32; 2], ppp: f32, f: &Frame) {
        let screen = Vec2::new(size_px[0] as f32 / ppp, size_px[1] as f32 / ppp);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
            viewports: std::iter::once((egui::ViewportId::ROOT, egui::ViewportInfo { native_pixels_per_point: Some(ppp), ..Default::default() })).collect(),
            ..Default::default()
        };
        let logo = (self.logo.id(), self.logo.size_vec2());
        let mut out = self.ctx.run_ui(raw, |ui| paint(ui, screen, logo, f));
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas.iter() {
                self.renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        let free: Vec<egui::TextureId> = out.textures_delta.free.iter().copied().collect();
        out.textures_delta.clear();
        let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: size_px, pixels_per_point: out.pixels_per_point };
        let extra = self.renderer.update_buffers(&gpu.device, &gpu.queue, enc, &prims, &sd);
        if !extra.is_empty() {
            gpu.queue.submit(extra);
        }
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("indicatori"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &prims, &sd);
        }
        for id in &free {
            self.renderer.free_texture(id);
        }
    }
}

fn white(alpha: f32, opacity: f32) -> Color32 {
    Color32::from_white_alpha((alpha * opacity * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// Testo "ETICHETTA valore" con l'etichetta più piccola e più tenue.
fn labeled(ui: &egui::Ui, label: &str, value: &str, size: f32, alpha: f32, op: f32) -> Arc<Galley> {
    let mut job = LayoutJob::default();
    if !label.is_empty() {
        job.append(label, 0.0, TextFormat { font_id: FontId::proportional(size - 2.0), color: white(alpha * 0.66, op), valign: egui::Align::Center, ..Default::default() });
    }
    let lead = if label.is_empty() { 0.0 } else { 4.0 };
    job.append(value, lead, TextFormat { font_id: FontId::proportional(size), color: white(alpha, op), valign: egui::Align::Center, ..Default::default() });
    ui.painter().layout_job(job)
}

/// Etichetta e valore di CPU / RAM (es. "CPU", "23%").
fn label_value(item: Item, f: &Frame) -> (&'static str, String) {
    match item {
        Item::Cpu => ("CPU", indicators::fmt_cpu(f.stats.cpu).trim_start_matches("CPU ").to_string()),
        Item::Ram => ("RAM", indicators::fmt_ram(f.stats.ram_used, f.stats.ram_total).trim_start_matches("RAM ").to_string()),
        Item::Clock => ("", indicators::fmt_clock(f.hm.0, f.hm.1)),
        Item::Logo => ("", String::new()),
    }
}

fn prepare(ui: &egui::Ui, block: &Block, f: &Frame) -> Piece {
    let op = f.opacity;
    match block {
        Block::Single(Item::Logo) => Piece::Logo,
        Block::Single(item) => {
            let (label, value) = label_value(*item, f);
            let big_clock = *item == Item::Clock;
            let g = match f.ind.style {
                IndicatorStyle::Text => {
                    if big_clock {
                        labeled(ui, "", &value, 20.0, 0.9, op)
                    } else {
                        labeled(ui, label, &value, 12.0, 0.75, op)
                    }
                }
                IndicatorStyle::Pills => labeled(ui, label, &value, 12.0, 0.95, op),
                IndicatorStyle::BigClock => labeled(ui, label, &value, 11.0, 0.7, op),
            };
            Piece::Text(g)
        }
        Block::ClockStack(rows) => {
            let (_, value) = label_value(Item::Clock, f);
            let clock = labeled(ui, "", &value, 30.0, 0.95, op);
            let rows = rows
                .iter()
                .map(|i| {
                    let (l, v) = label_value(*i, f);
                    labeled(ui, l, &v, 11.0, 0.7, op)
                })
                .collect();
            Piece::Stack { clock, rows }
        }
    }
}

fn piece_size(p: &Piece, logo: Vec2, pills: bool) -> Vec2 {
    let s = match p {
        Piece::Logo => Vec2::new(LOGO_H * logo.x / logo.y.max(1.0), LOGO_H),
        Piece::Text(g) => g.size(),
        Piece::Stack { clock, rows } => {
            let w = rows.iter().map(|r| r.size().x).fold(clock.size().x, f32::max);
            let h = clock.size().y + rows.iter().map(|r| r.size().y + 1.0).sum::<f32>();
            Vec2::new(w, h)
        }
    };
    if pills { s + PILL_PAD * 2.0 } else { s }
}

fn paint(ui: &mut egui::Ui, screen: Vec2, logo: (egui::TextureId, Vec2), f: &Frame) {
    let pills = f.ind.style == IndicatorStyle::Pills;
    let gap = if pills { 6.0 } else { 14.0 };
    let (free_l, free_r) = indicators::free_widths(screen.x, f.dock);
    let painter = ui.painter().clone();
    for (side, free) in [(Side::Left, free_l), (Side::Right, free_r)] {
        let pieces: Vec<Piece> = indicators::blocks(f.ind, side).iter().map(|b| prepare(ui, b, f)).collect();
        let sizes: Vec<Vec2> = pieces.iter().map(|p| piece_size(p, logo.1, pills)).collect();
        let n = indicators::fit(&sizes.iter().map(|s| s.x).collect::<Vec<_>>(), free, gap);
        if n == 0 {
            continue;
        }
        let max_h = sizes[..n].iter().map(|s| s.y).fold(0.0, f32::max);
        let cy = indicators::center_y(f.dock, screen.y, max_h);
        let mut x = if side == Side::Left { EDGE_MARGIN } else { screen.x - EDGE_MARGIN };
        for (p, s) in pieces.iter().zip(&sizes).take(n) {
            let x0 = if side == Side::Left { x } else { x - s.x };
            let rect = Rect::from_min_size(Pos2::new(x0, cy - s.y / 2.0), *s);
            draw_piece(&painter, p, rect, side, logo.0, pills, f.opacity);
            x = if side == Side::Left { x + s.x + gap } else { x - s.x - gap };
        }
    }
}

fn draw_piece(painter: &egui::Painter, p: &Piece, rect: Rect, side: Side, logo: egui::TextureId, pills: bool, op: f32) {
    let inner = if pills {
        painter.rect(rect, 10.0, white(0.10, op), Stroke::new(1.0, white(0.16, op)), egui::StrokeKind::Inside);
        rect.shrink2(PILL_PAD)
    } else {
        rect
    };
    match p {
        Piece::Logo => {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            painter.image(logo, inner, uv, white(1.0, op));
        }
        Piece::Text(g) => painter.galley(inner.min, g.clone(), Color32::WHITE),
        Piece::Stack { clock, rows } => {
            // allineato verso il bordo dello schermo
            let at = |w: f32| if side == Side::Left { inner.min.x } else { inner.max.x - w };
            let mut y = inner.min.y;
            painter.galley(Pos2::new(at(clock.size().x), y), clock.clone(), Color32::WHITE);
            y += clock.size().y;
            for r in rows {
                y += 1.0;
                painter.galley(Pos2::new(at(r.size().x), y), r.clone(), Color32::WHITE);
                y += r.size().y;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn logo_ridotto_una_volta() {
        let img = super::logo_image();
        assert_eq!(img.height(), (super::LOGO_H * 3.0) as u32);
        assert!(img.width() > img.height() * 3);
    }
}
