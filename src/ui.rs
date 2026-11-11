//! Finestre dell'interfaccia (egui): avviso call, pausa caffè, richiesta di conferma,
//! impostazioni. Esistono solo quando sono visibili, così l'app resta leggera.

use crate::config::Config;
use crate::theme::{self, ic, Kind};
use chrono::{Datelike, NaiveDate};
use crate::detect::MicUser;
use crate::office::Office;
use crate::render::{Gpu, Target};
use egui::{Align, Color32, Layout, RichText, Stroke, Vec2};
use std::sync::Arc;
use std::time::{Duration, Instant};
#[cfg(target_os = "macos")]
use winit::dpi::LogicalPosition;
use winit::dpi::{LogicalSize, Position, Size};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowAttributes, WindowLevel};

// ---------- palette (alias dei colori del tema, usati anche da statsui) ----------
pub const INK: Color32 = theme::BG;
pub const RED_SOFT: Color32 = theme::RED_TEXT;
pub const TEXT: Color32 = theme::TEXT;
pub const MUTED: Color32 = theme::TEXT_2;
/// Fondo delle finestre di avviso.
pub const PANEL_BG: Color32 = theme::BG_SIDE;

/// Azioni scelte dall'utente nelle finestre, eseguite poi dall'app.
#[derive(Clone, Debug)]
pub enum UiAction {
    DismissCalls,
    EndMyCall,
    Rsvp(bool),
    EndCoffee,
    DismissCoffee,
    AskYes,
    AskNo,
    Ignore(String),
    Unignore(String),
    ConfigChanged,
    OpenUpdate,
    CheckUpdates,
    InstallUpdate,
    /// Apre la finestra "Le mie call".
    OpenCallStats,
    /// Calcola quante call toglierebbe la cancellazione (riempie `DeleteForm::confirm`).
    PreviewDelete,
    /// Cancella davvero l'intervallo scelto.
    DeleteCalls,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    OpenAccessibility,
    /// Una voce che esiste anche nel menu dell'icona (sfondi, call, caffè, uscita…).
    Menu(crate::tray::Action),
    /// Suona l'avviso al volume scelto.
    TestSound,
    /// Chiude il menu a comparsa dell'icona.
    ClosePopup,
    /// Apre la cartella con log, impostazioni e storico.
    OpenAppDir,
    /// Apre le Impostazioni su una pagina.
    OpenSettings(Page),
}

/// Pagine della finestra Impostazioni.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Home,
    Calls,
    Alerts,
    Wallpaper,
    MyCalls,
    Diagnostics,
    About,
}

/// Pagine del menu a comparsa dell'icona.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TrayPage {
    #[default]
    Main,
    Coffee,
    Wallpaper,
}

/// Dati che le finestre possono leggere (e, per le impostazioni, modificare).
pub struct View<'a> {
    pub cfg: &'a mut Config,
    pub office: &'a Office,
    pub me: String,
    pub mic: &'a [MicUser],
    pub ask: Option<&'a str>,
    pub update: &'a crate::update::UpdateState,
    pub peers: Vec<String>,
    /// Permesso Accessibilità (posizione esatta del Dock per gli indicatori, solo macOS).
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    pub ax_trusted: bool,
    /// Modulo "Cancella lo storico…" (resta all'app tra un disegno e l'altro).
    pub delete: &'a mut DeleteForm,
    /// Primo giorno con una call registrata, per il pulsante "Tutto".
    pub first_call_day: Option<NaiveDate>,
    /// Pagina aperta nelle Impostazioni e nel menu a comparsa.
    pub page: &'a mut Page,
    pub tray_page: &'a mut TrayPage,
    /// Settimana in corso delle mie call (per la panoramica) e totale di oggi.
    pub week: Option<&'a crate::callstats::Report>,
    pub today_ms: f64,
    pub autostart: bool,
    pub catalog: &'a [crate::wallpapers::Wallpaper],
    pub monitors: &'a [crate::tray::MonitorInfo],
    /// Miniature degli sfondi per la galleria (id, texture).
    pub thumbs: &'a [(String, Arc<egui::ColorImage>)],
    pub actions: Vec<UiAction>,
}

/// Stato del modulo "Cancella lo storico…": intervallo scelto (estremi compresi) e, dopo
/// `PreviewDelete`, quante call e quanto tempo verrebbero cancellati.
pub struct DeleteForm {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub open: bool,
    pub confirm: Option<(usize, f64)>,
}

impl DeleteForm {
    /// Modulo chiuso; di default dal primo giorno del mese a oggi.
    pub fn new(today: NaiveDate) -> Self {
        Self { from: today.with_day(1).unwrap_or(today), to: today, open: false, confirm: None }
    }
}

// ============================================================================
// Finestra egui generica
// ============================================================================

pub struct UiWin {
    pub window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    ctx: egui::Context,
    state: egui_winit::State,
    renderer: egui_wgpu::Renderer,
    pub next_paint: Instant,
    shown: bool,
    /// Finestra normale (impostazioni): quando appare va portata in primo piano.
    focus_on_show: bool,
}

impl UiWin {
    pub fn new(el: &ActiveEventLoop, gpu: &Gpu, attrs: WindowAttributes, overlay: bool) -> Option<Self> {
        let window = Arc::new(el.create_window(attrs.with_visible(false)).ok()?);
        if overlay {
            crate::platform::make_overlay(&window);
            crate::platform::style_popup(&window, theme::BORDER);
        } else {
            crate::platform::style_window(&window);
        }
        let surface = gpu.instance.create_surface(window.clone()).ok()?;
        let format = gpu.surface_format(&surface);
        let size = window.inner_size();
        let caps = surface.get_capabilities(&gpu.adapter);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 1,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            color_space: wgpu::SurfaceColorSpace::Auto,
        };
        surface.configure(&gpu.device, &config);
        let ctx = egui::Context::default();
        setup_style(&ctx);
        let state = egui_winit::State::new(
            ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            Some(gpu.device.limits().max_texture_dimension_2d as usize),
        );
        let renderer = egui_wgpu::Renderer::new(&gpu.device, format, egui_wgpu::RendererOptions::default());
        Some(Self { window, surface, config, ctx, state, renderer, next_paint: Instant::now(), shown: false, focus_on_show: !overlay })
    }

    pub fn on_event(&mut self, gpu: &Gpu, event: &WindowEvent) {
        if let WindowEvent::Resized(s) = event {
            self.config.width = s.width.max(1);
            self.config.height = s.height.max(1);
            self.surface.configure(&gpu.device, &self.config);
        }
        if self.state.on_window_event(&self.window, event).repaint {
            self.next_paint = Instant::now();
        }
    }

    pub fn paint(&mut self, gpu: &Gpu, bg: Color32, build: impl FnMut(&mut egui::Ui)) {
        let mut build = build;
        let raw = self.state.take_egui_input(&self.window);
        let mut out = self.ctx.run_ui(raw, |ui| build(ui));
        self.state.handle_platform_output(&self.window, out.platform_output);
        let ppp = out.pixels_per_point;
        let prims = self.ctx.tessellate(out.shapes, ppp);
        let delay = out
            .viewport_output
            .get(&egui::ViewportId::ROOT)
            .map(|v| v.repaint_delay)
            .unwrap_or(Duration::from_secs(1))
            .min(Duration::from_secs(1));
        self.next_paint = Instant::now() + delay.max(Duration::from_millis(30));

        // La finestra va resa visibile prima di chiedere il fotogramma: su macOS una finestra
        // nascosta non riceve mai un fotogramma da disegnare, e resterebbe nascosta per sempre.
        if !self.shown {
            self.shown = true;
            self.window.set_visible(true);
            if self.focus_on_show {
                // Su macOS porta in primo piano anche l'app (che vive solo nella barra dei menu).
                self.window.focus_window();
            }
            self.next_paint = Instant::now() + Duration::from_millis(16);
        }
        // Le texture (es. i caratteri) vanno caricate sempre, anche se questo fotogramma
        // non si può disegnare: egui non le rimanda una seconda volta.
        let mut textures = std::mem::take(&mut out.textures_delta);
        for (id, deltas) in &textures.set {
            for delta in deltas.iter() {
                self.renderer.update_texture(&gpu.device, &gpu.queue, *id, delta);
            }
        }
        let free: Vec<egui::TextureId> = textures.free.iter().copied().collect();
        textures.clear();
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            other => {
                if !matches!(other, wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded) {
                    self.surface.configure(&gpu.device, &self.config);
                }
                for id in &free {
                    self.renderer.free_texture(id);
                }
                // riprova a breve
                self.next_paint = Instant::now() + Duration::from_millis(50);
                return;
            }
        };
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [self.config.width, self.config.height], pixels_per_point: ppp };
        let view = frame.texture.create_view(&Default::default());
        let mut enc = gpu.device.create_command_encoder(&Default::default());
        let extra = self.renderer.update_buffers(&gpu.device, &gpu.queue, &mut enc, &prims, &sd);
        {
            let c = bg.to_normalized_gamma_f32();
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("egui"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            self.renderer.render(&mut pass, &prims, &sd);
        }
        gpu.queue.submit(extra.into_iter().chain([enc.finish()]));
        gpu.queue.present(frame);
        for id in &free {
            self.renderer.free_texture(id);
        }
    }

    pub fn focus(&self) {
        self.window.set_visible(true);
        self.window.focus_window();
    }

    pub fn set_height(&self, h: f64) {
        let cur = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        if (cur.height - h).abs() > 1.0 {
            let _ = self.window.request_inner_size(LogicalSize::new(cur.width, h));
        }
    }
}

pub fn setup_style(ctx: &egui::Context) {
    theme::install(ctx);
}

// ============================================================================
// Posizionamento delle finestre sugli schermi
// ============================================================================

/// Attributi per una finestra in un rettangolo (in punti logici) relativo allo schermo `m`.
pub fn attrs_on(m: &MonitorHandle, x: f64, y: f64, w: f64, h: f64) -> WindowAttributes {
    let s = m.scale_factor();
    let origin = m.position();
    // Su Windows le coordinate sono fisiche e ogni schermo ha la sua scala;
    // su macOS lo spazio è in punti.
    #[cfg(target_os = "macos")]
    let (pos, size): (Position, Size) = {
        let o = origin.to_logical::<f64>(s);
        (LogicalPosition::new(o.x + x, o.y + y).into(), LogicalSize::new(w, h).into())
    };
    #[cfg(not(target_os = "macos"))]
    let (pos, size): (Position, Size) = (
        winit::dpi::PhysicalPosition::new(origin.x + (x * s) as i32, origin.y + (y * s) as i32).into(),
        winit::dpi::PhysicalSize::new((w * s) as u32, (h * s) as u32).into(),
    );
    Window::default_attributes().with_position(pos).with_inner_size(size)
}

pub fn overlay_attrs(a: WindowAttributes, title: &str) -> WindowAttributes {
    #[allow(unused_mut)]
    let mut a = a
        .with_title(title)
        .with_decorations(false)
        .with_resizable(false)
        .with_window_level(WindowLevel::AlwaysOnTop)
        .with_active(false);
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        a = a.with_skip_taskbar(true);
    }
    a
}

/// Dimensione logica di uno schermo.
pub fn logical_size(m: &MonitorHandle) -> (f64, f64) {
    let s = m.size().to_logical::<f64>(m.scale_factor());
    (s.width, s.height)
}

// ============================================================================
// Bordo rosso pulsante (quattro finestre sottili per schermo, come Call Alert)
// ============================================================================

pub struct Edge {
    pub target: Target,
}

pub fn make_edges(el: &ActiveEventLoop, gpu: &Gpu, monitors: &[MonitorHandle]) -> Vec<Edge> {
    let mut v = Vec::new();
    let t = 6.0;
    for m in monitors {
        let (w, h) = logical_size(m);
        for (x, y, ew, eh) in [(0.0, 0.0, w, t), (0.0, h - t, w, t), (0.0, t, t, h - 2.0 * t), (w - t, t, t, h - 2.0 * t)] {
            let attrs = overlay_attrs(attrs_on(m, x, y, ew, eh), "DevLogica Tools – call");
            let Ok(win) = el.create_window(attrs) else { continue };
            let win = Arc::new(win);
            crate::platform::make_overlay(&win);
            let Ok(surface) = gpu.instance.create_surface(win.clone()) else { continue };
            v.push(Edge { target: gpu.make_target(win, surface, 1.0) });
        }
    }
    v
}

pub fn edge_color(t: f64) -> wgpu::Color {
    // pulsazione tra rosso e arancio-rosso, ogni 1,4 secondi
    let k = 0.5 + 0.5 * (t * std::f64::consts::TAU / 1.4).sin();
    let mix = |a: f64, b: f64| (a + (b - a) * k) / 255.0;
    wgpu::Color { r: mix(255.0, 255.0), g: mix(59.0, 107.0), b: mix(48.0, 53.0), a: 1.0 }
}

// ============================================================================
// Contenuti
// ============================================================================

fn mmss(start_ms: f64) -> String {
    let secs = ((crate::config::now_ms() - start_ms) / 1000.0).max(0.0) as u64;
    if secs >= 3600 {
        format!("{}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
    } else {
        format!("{:02}:{:02}", secs / 60, secs % 60)
    }
}

/// Cornice comune delle finestre di avviso: bordo sottile e, sopra, una riga colorata.
fn alert_frame(ui: &mut egui::Ui, accent: Color32, add: impl FnOnce(&mut egui::Ui)) {
    let rect = ui.max_rect();
    let p = ui.painter();
    p.rect_stroke(rect.shrink(0.5), theme::R_MD, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    p.rect_filled(egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0)), egui::CornerRadius { nw: theme::R_MD, ne: theme::R_MD, sw: 0, se: 0 }, accent);
    egui::Frame::new().inner_margin(egui::Margin { left: 20, right: 16, top: 16, bottom: 14 }).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        add(ui)
    });
}

/// Cerchio colorato con un'icona al centro.
fn icon_badge(ui: &mut egui::Ui, code: &str, fg: Color32, bg: Color32, size: f32) {
    let (r, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    ui.painter().circle_filled(r.center(), size / 2.0, bg);
    theme::paint_icon(ui.painter(), r.center(), code, size * 0.42, fg);
}

const CALL_ROW: f32 = 58.0;

/// Altezza del riquadro call in base al numero di call mostrate.
pub fn call_panel_height(n: usize) -> f64 {
    80.0 + CALL_ROW as f64 * n as f64
}

pub fn call_panel(ui: &mut egui::Ui, v: &mut View) {
    let mut calls: Vec<_> = v.office.calls.values().collect();
    calls.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    alert_frame(ui, theme::RED, |ui| {
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            theme::status_dot(ui, theme::RED, true);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 1.0;
                let n = calls.len();
                ui.label(RichText::new(if n == 1 { "CALL IN CORSO".to_string() } else { format!("{n} CALL IN CORSO") }).family(theme::semibold_family()).size(11.0).color(theme::RED_TEXT).extra_letter_spacing(0.8));
                ui.label(theme::strong("Silenzio, per favore", 17.0));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::button(ui, Kind::Secondary, "Ho capito").clicked() {
                    v.actions.push(UiAction::DismissCalls);
                }
            });
        });
        ui.add_space(4.0);
        ui.spacing_mut().item_spacing.y = 0.0;
        for c in &calls {
            let (row, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), CALL_ROW), egui::Sense::hover());
            let p = ui.painter();
            p.hline(row.left()..=row.right(), row.top(), Stroke::new(1.0, theme::BORDER_SOFT));
            let cy = row.center().y;
            theme::paint_avatar(p, egui::pos2(row.left() + 17.0, cy), &c.user, 34.0);
            let x = row.left() + 46.0;
            let name = p.layout_no_wrap(c.user.clone(), egui::FontId::new(14.5, theme::semibold_family()), theme::TEXT);
            let name_w = name.size().x;
            p.galley(egui::pos2(x, cy - 19.0), name, theme::TEXT);
            if c.mine {
                p.text(egui::pos2(x + name_w + 6.0, cy - 10.5), egui::Align2::LEFT_CENTER, "tu", egui::FontId::proportional(12.0), theme::TEXT_3);
            }
            theme::paint_icon(p, egui::pos2(x + 6.0, cy + 10.0), ic::PHONE, 11.0, theme::TEXT_3);
            let app_x = if theme::has_icons() { x + 18.0 } else { x };
            p.text(egui::pos2(app_x, cy + 10.0), egui::Align2::LEFT_CENTER, c.app.clone().unwrap_or_else(|| "In call".into()), egui::FontId::proportional(12.5), theme::TEXT_2);
            let mut right = row.right();
            if c.mine {
                let br = egui::Rect::from_min_max(egui::pos2(right - 86.0, cy - 16.0), egui::pos2(right, cy + 16.0));
                let clicked = ui.scope_builder(egui::UiBuilder::new().max_rect(br).layout(Layout::right_to_left(Align::Center)), |ui| theme::button(ui, Kind::Danger, "Termina").clicked()).inner;
                if clicked {
                    v.actions.push(UiAction::EndMyCall);
                }
                right -= 98.0;
            }
            ui.painter().text(egui::pos2(right, cy), egui::Align2::RIGHT_CENTER, mmss(c.start_ms), egui::FontId::new(15.0, theme::semibold_family()), theme::TEXT);
        }
    });
    ui.ctx().request_repaint_after(Duration::from_millis(250));
}

pub fn coffee_panel_height(rows: usize, mine: bool) -> f64 {
    let answer = if mine { 0.0 } else { 54.0 };
    196.0 + answer + 38.0 * rows as f64
}

pub fn coffee_panel(ui: &mut egui::Ui, v: &mut View) {
    let Some(c) = v.office.coffee.as_ref() else { return };
    let mine = c.mine;
    let my_answer = c.rsvp.get(&v.me).cloned();
    alert_frame(ui, theme::AMBER, |ui| {
        ui.horizontal(|ui| {
            icon_badge(ui, ic::COFFEE, theme::AMBER, theme::AMBER_BG, 42.0);
            ui.add_space(4.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(theme::strong("Pausa caffè", 18.0));
                let who = if mine { "Proposta da te".to_string() } else { format!("Proposta da {}", c.by) };
                ui.label(RichText::new(who).size(13.0).color(theme::TEXT_2));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let when = match c.in_minutes {
                    Some(m) if m > 0 => format!("Tra {m} min"),
                    _ => "Adesso".to_string(),
                };
                theme::chip(ui, &when, theme::AMBER, theme::AMBER_BG);
            });
        });
        ui.add_space(14.0);
        if !mine {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 8.0) / 2.0;
                let yes_on = my_answer.as_deref() == Some("yes");
                let no_on = my_answer.as_deref() == Some("no");
                let yes = egui::Button::new(RichText::new(format!("{}  Vengo", if theme::has_icons() && yes_on { ic::CHECK } else { "" })).family(theme::semibold_family()).size(14.0).color(if yes_on { Color32::from_rgb(30, 20, 6) } else { theme::TEXT }))
                    .fill(if yes_on { theme::AMBER } else { theme::SURFACE_HI })
                    .stroke(if yes_on { Stroke::NONE } else { Stroke::new(1.0, theme::BORDER) })
                    .corner_radius(theme::R_SM);
                if ui.add_sized([w, 38.0], yes).clicked() {
                    v.actions.push(UiAction::Rsvp(true));
                }
                let no = egui::Button::new(RichText::new("Non posso").family(theme::semibold_family()).size(14.0).color(if no_on { theme::RED_TEXT } else { theme::TEXT }))
                    .fill(if no_on { theme::RED_BG } else { theme::SURFACE_HI })
                    .stroke(Stroke::new(1.0, if no_on { Color32::from_rgb(92, 40, 42) } else { theme::BORDER }))
                    .corner_radius(theme::R_SM);
                if ui.add_sized([w, 38.0], no).clicked() {
                    v.actions.push(UiAction::Rsvp(false));
                }
            });
            ui.add_space(14.0);
        }
        let yes_n = c.rsvp.values().filter(|a| a.as_str() == "yes").count();
        ui.horizontal(|ui| {
            ui.label(theme::eyebrow("Risposte"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(RichText::new(if yes_n == 1 { "1 viene".to_string() } else { format!("{yes_n} vengono") }).size(12.0).color(theme::TEXT_3));
            });
        });
        ui.add_space(2.0);
        if c.rsvp.is_empty() {
            ui.add_space(6.0);
            ui.label(RichText::new("Ancora nessuna risposta").size(13.0).color(theme::TEXT_3));
        }
        for (user, answer) in &c.rsvp {
            ui.horizontal(|ui| {
                ui.set_min_height(30.0);
                theme::avatar(ui, user, 26.0);
                ui.label(RichText::new(user).size(14.0).color(theme::TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    match answer.as_str() {
                        "yes" => theme::chip(ui, "Viene", theme::GREEN, theme::GREEN_BG),
                        crate::office::IN_CALL => theme::chip(ui, "In call", theme::TEXT_2, theme::SURFACE_HI),
                        _ => theme::chip(ui, "Non può", theme::RED_TEXT, theme::RED_BG),
                    };
                });
            });
        }
        ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
            if mine {
                if theme::button(ui, Kind::Amber, "Termina la pausa").clicked() {
                    v.actions.push(UiAction::EndCoffee);
                }
            } else if theme::button(ui, Kind::Secondary, "Chiudi").clicked() {
                v.actions.push(UiAction::DismissCoffee);
            }
        });
    });
}

pub fn ask_panel(ui: &mut egui::Ui, v: &mut View) {
    let app = v.ask.unwrap_or("un'app di call");
    alert_frame(ui, theme::ACCENT, |ui| {
        ui.horizontal(|ui| {
            icon_badge(ui, ic::MIC, theme::ACCENT_TEXT, theme::ACCENT_BG, 38.0);
            ui.add_space(2.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(theme::strong("Sei entrato in call?", 15.5));
                ui.label(RichText::new(format!("Il microfono è in uso da {app}.")).size(12.5).color(theme::TEXT_2));
            });
        });
        ui.with_layout(Layout::bottom_up(Align::Max), |ui| {
            ui.horizontal(|ui| {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::button(ui, Kind::Primary, "Avvisa l'ufficio").clicked() {
                        v.actions.push(UiAction::AskYes);
                    }
                    if theme::button(ui, Kind::Secondary, "No").clicked() {
                        v.actions.push(UiAction::AskNo);
                    }
                });
            });
        });
    });
}

pub fn section(ui: &mut egui::Ui, title: &str) {
    theme::section_title(ui, title);
}

fn to_jiff(d: NaiveDate) -> jiff::civil::Date {
    jiff::civil::Date::new(d.year() as i16, d.month() as i8, d.day() as i8).unwrap_or_default()
}

fn from_jiff(d: jiff::civil::Date, prima: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year() as i32, d.month() as u32, d.day() as u32).unwrap_or(prima)
}

/// Selettore di una data (gg/mm/aaaa). Restituisce true se è cambiata.
pub fn date_field(ui: &mut egui::Ui, id: &str, d: &mut NaiveDate) -> bool {
    let mut j = to_jiff(*d);
    ui.add(egui_extras::DatePickerButton::new(&mut j).id_salt(id).format("%d/%m/%Y").calendar_week(false));
    let nuova = from_jiff(j, *d);
    let cambiata = nuova != *d;
    *d = nuova;
    cambiata
}


/// Disegna un riquadro in un'immagine, senza finestre (anteprime e verifiche: --ui).
pub fn render_preview(gpu: &Gpu, w: u32, h: u32, ppp: f32, bg: Color32, mut build: impl FnMut(&mut egui::Ui)) -> Vec<u8> {
    let ctx = egui::Context::default();
    setup_style(&ctx);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = egui_wgpu::Renderer::new(&gpu.device, format, egui_wgpu::RendererOptions::default());
    let input = || egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, Vec2::new(w as f32 / ppp, h as f32 / ppp))),
        viewports: std::iter::once((egui::ViewportId::ROOT, egui::ViewportInfo { native_pixels_per_point: Some(ppp), ..Default::default() })).collect(),
        ..Default::default()
    };
    // due passaggi: il primo serve a egui per misurare i contenuti
    let mut first = ctx.run_ui(input(), |ui| build(ui));
    let mut out = ctx.run_ui(input(), |ui| build(ui));
    for td in [&mut first.textures_delta, &mut out.textures_delta] {
        for (id, deltas) in &td.set {
            for d in deltas.iter() {
                renderer.update_texture(&gpu.device, &gpu.queue, *id, d);
            }
        }
        td.clear();
    }
    let prims = ctx.tessellate(std::mem::take(&mut out.shapes), out.pixels_per_point);
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("anteprima"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = tex.create_view(&Default::default());
    let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: out.pixels_per_point };
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    let extra = renderer.update_buffers(&gpu.device, &gpu.queue, &mut enc, &prims, &sd);
    {
        let c = bg.to_normalized_gamma_f32();
        let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color { r: c[0] as f64, g: c[1] as f64, b: c[2] as f64, a: 1.0 }), store: wgpu::StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let mut pass = pass.forget_lifetime();
        renderer.render(&mut pass, &prims, &sd);
    }
    let row = (w * 4).div_ceil(256) * 256;
    let buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (row * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(row), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    gpu.queue.submit(extra.into_iter().chain([enc.finish()]));
    let slice = buf.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
    let Ok(data) = slice.get_mapped_range() else { return Vec::new() };
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        px.extend_from_slice(&data[(y * row) as usize..(y * row + w * 4) as usize]);
    }
    px
}
