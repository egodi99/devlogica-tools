//! Finestre dell'interfaccia (egui): avviso call, pausa caffè, richiesta di conferma,
//! impostazioni. Esistono solo quando sono visibili, così l'app resta leggera.

use crate::config::{AlertStyle, Config, Detection};
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

// ---------- palette ----------
pub const INK: Color32 = Color32::from_rgb(14, 9, 28);
const INK_2: Color32 = Color32::from_rgb(26, 18, 46);
const LILAC: Color32 = Color32::from_rgb(194, 136, 214);
const SKY: Color32 = Color32::from_rgb(112, 189, 232);
const RED: Color32 = Color32::from_rgb(255, 59, 48);
pub const RED_BG: Color32 = Color32::from_rgb(28, 7, 9);
const RED_SOFT: Color32 = Color32::from_rgb(255, 180, 170);
const AMBER: Color32 = Color32::from_rgb(240, 170, 70);
pub const COFFEE_BG: Color32 = Color32::from_rgb(26, 17, 10);
const GREEN: Color32 = Color32::from_rgb(52, 199, 89);
const TEXT: Color32 = Color32::from_rgb(241, 234, 247);
const MUTED: Color32 = Color32::from_rgb(170, 160, 190);

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
}

/// Dati che le finestre possono leggere (e, per le impostazioni, modificare).
pub struct View<'a> {
    pub cfg: &'a mut Config,
    pub office: &'a Office,
    pub me: String,
    pub mic: &'a [MicUser],
    pub ask: Option<&'a str>,
    pub update: Option<&'a str>,
    pub peers: Vec<String>,
    pub actions: Vec<UiAction>,
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
}

impl UiWin {
    pub fn new(el: &ActiveEventLoop, gpu: &Gpu, attrs: WindowAttributes, overlay: bool) -> Option<Self> {
        let window = Arc::new(el.create_window(attrs.with_visible(false)).ok()?);
        if overlay {
            crate::platform::make_overlay(&window);
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
        Some(Self { window, surface, config, ctx, state, renderer, next_paint: Instant::now(), shown: false })
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
        if !self.shown {
            self.shown = true;
            self.window.set_visible(true);
        }
    }

    pub fn set_height(&self, h: f64) {
        let cur = self.window.inner_size().to_logical::<f64>(self.window.scale_factor());
        if (cur.height - h).abs() > 1.0 {
            let _ = self.window.request_inner_size(LogicalSize::new(cur.width, h));
        }
    }
}

pub fn setup_style(ctx: &egui::Context) {
    let mut v = egui::Visuals::dark();
    v.panel_fill = INK;
    v.window_fill = INK;
    v.extreme_bg_color = INK_2;
    v.selection.bg_fill = Color32::from_rgb(134, 18, 173);
    v.hyperlink_color = SKY;
    v.widgets.inactive.weak_bg_fill = Color32::from_rgb(44, 32, 70);
    v.widgets.hovered.weak_bg_fill = Color32::from_rgb(64, 46, 100);
    v.widgets.active.weak_bg_fill = Color32::from_rgb(86, 60, 130);
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
    });
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

fn avatar(ui: &mut egui::Ui, name: &str, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 18.0, color);
    let initial = name.chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, initial, egui::FontId::proportional(17.0), Color32::WHITE);
}

fn button(ui: &mut egui::Ui, text: &str, fill: Color32, fg: Color32) -> bool {
    ui.add(egui::Button::new(RichText::new(text).color(fg).size(14.0)).fill(fill).corner_radius(8.0)).clicked()
}

/// Altezza del riquadro call in base al numero di call mostrate.
pub fn call_panel_height(n: usize) -> f64 {
    96.0 + 56.0 * n as f64
}

pub fn call_panel(ui: &mut egui::Ui, v: &mut View) {
    let calls: Vec<_> = v.office.calls.values().collect();
    let mut calls = calls;
    calls.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    egui::Frame::new().fill(RED_BG).stroke(Stroke::new(2.0, RED)).inner_margin(16.0).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.horizontal(|ui| {
            // pallino che pulsa
            let t = ui.input(|i| i.time);
            let (r, _) = ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
            let k = (0.5 + 0.5 * (t * std::f64::consts::TAU / 1.4).sin()) as f32;
            ui.painter().circle_filled(r.center(), 16.0 + 3.0 * k, RED.gamma_multiply(0.25 + 0.25 * k));
            ui.painter().circle_filled(r.center(), 8.0, RED);
            ui.vertical(|ui| {
                ui.label(RichText::new("CALL IN CORSO — Silenzio!").size(20.0).strong().color(TEXT));
                let n = calls.len();
                ui.label(RichText::new(if n == 1 { "1 call attiva".to_string() } else { format!("{n} call attive") }).size(12.0).color(RED_SOFT));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if button(ui, "Ho capito", Color32::from_white_alpha(18), TEXT) {
                    v.actions.push(UiAction::DismissCalls);
                }
            });
        });
        ui.add_space(6.0);
        for c in &calls {
            egui::Frame::new().fill(Color32::from_rgb(40, 12, 14)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    avatar(ui, &c.user, Color32::from_rgb(230, 70, 50));
                    ui.vertical(|ui| {
                        let who = if c.mine { format!("{} (tu)", c.user) } else { c.user.clone() };
                        ui.label(RichText::new(who).size(15.0).strong().color(TEXT));
                        ui.label(RichText::new(c.app.clone().unwrap_or_else(|| "In call".into())).size(12.0).color(RED_SOFT));
                    });
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if c.mine && button(ui, "Termina", Color32::from_rgb(90, 20, 20), Color32::from_rgb(255, 120, 110)) {
                            v.actions.push(UiAction::EndMyCall);
                        }
                        ui.label(RichText::new(mmss(c.start_ms)).size(15.0).monospace().color(Color32::from_rgb(255, 130, 120)));
                    });
                });
            });
        }
    });
    ui.ctx().request_repaint_after(Duration::from_millis(250));
}

pub fn coffee_panel_height(rows: usize) -> f64 {
    232.0 + 26.0 * rows as f64
}

pub fn coffee_panel(ui: &mut egui::Ui, v: &mut View) {
    let Some(c) = v.office.coffee.as_ref() else { return };
    let mine = c.mine;
    let my_answer = c.rsvp.get(&v.me).cloned();
    egui::Frame::new().fill(COFFEE_BG).stroke(Stroke::new(2.0, AMBER)).inner_margin(18.0).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("Pausa caffè").size(24.0).strong().color(AMBER));
            let when = match c.in_minutes {
                Some(m) if m > 0 => format!("tra {m} minuti"),
                _ => "adesso".to_string(),
            };
            let who = if mine { "Proposta da te".to_string() } else { format!("Proposta da {}", c.by) };
            ui.label(RichText::new(format!("{who} · {when}")).size(14.0).color(TEXT));
        });
        ui.add_space(10.0);
        if !mine {
            ui.horizontal(|ui| {
                let w = (ui.available_width() - 8.0) / 2.0;
                let yes_on = my_answer.as_deref() == Some("yes");
                let no_on = my_answer.as_deref() == Some("no");
                if ui.add_sized([w, 36.0], egui::Button::new(RichText::new("Vengo!").size(15.0).color(if yes_on { Color32::BLACK } else { TEXT })).fill(if yes_on { GREEN } else { Color32::from_rgb(40, 60, 40) }).corner_radius(8.0)).clicked() {
                    v.actions.push(UiAction::Rsvp(true));
                }
                if ui.add_sized([w, 36.0], egui::Button::new(RichText::new("Non posso").size(15.0).color(TEXT)).fill(if no_on { Color32::from_rgb(150, 40, 40) } else { Color32::from_rgb(60, 32, 32) }).corner_radius(8.0)).clicked() {
                    v.actions.push(UiAction::Rsvp(false));
                }
            });
            ui.add_space(6.0);
        }
        ui.label(RichText::new("Risposte").size(12.0).color(MUTED));
        if c.rsvp.is_empty() {
            ui.label(RichText::new("Ancora nessuna risposta").size(13.0).italics().color(MUTED));
        }
        for (user, answer) in &c.rsvp {
            ui.horizontal(|ui| {
                let yes = answer == "yes";
                ui.label(RichText::new(if yes { "✔" } else { "✖" }).color(if yes { GREEN } else { RED }));
                ui.label(RichText::new(user).size(14.0).color(TEXT));
                ui.label(RichText::new(if yes { "viene" } else { "non può" }).size(12.0).color(MUTED));
            });
        }
        ui.with_layout(Layout::bottom_up(Align::Center), |ui| {
            if mine {
                if button(ui, "Fine pausa caffè", Color32::from_rgb(90, 60, 20), TEXT) {
                    v.actions.push(UiAction::EndCoffee);
                }
            } else if button(ui, "Chiudi", Color32::from_white_alpha(18), TEXT) {
                v.actions.push(UiAction::DismissCoffee);
            }
        });
    });
}

pub fn ask_panel(ui: &mut egui::Ui, v: &mut View) {
    let app = v.ask.unwrap_or("un'app di call");
    egui::Frame::new().fill(INK).stroke(Stroke::new(1.5, LILAC)).inner_margin(16.0).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        ui.label(RichText::new("Sei entrato in call?").size(17.0).strong().color(TEXT));
        ui.label(RichText::new(format!("Il microfono è in uso da {app}.")).size(13.0).color(MUTED));
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if button(ui, "Avvisa l'ufficio", Color32::from_rgb(134, 18, 173), Color32::WHITE) {
                v.actions.push(UiAction::AskYes);
            }
            if button(ui, "No", Color32::from_white_alpha(18), TEXT) {
                v.actions.push(UiAction::AskNo);
            }
        });
    });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(title).size(15.0).strong().color(LILAC));
}

pub fn settings(ui: &mut egui::Ui, v: &mut View) {
    let before = v.cfg.clone();
    egui::Frame::new().fill(INK).inner_margin(18.0).show(ui, |ui| {
        ui.set_min_size(ui.available_size());
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.label(RichText::new("DevLogica Tools").size(22.0).strong().color(TEXT));
            ui.label(RichText::new(format!("Versione {}", env!("CARGO_PKG_VERSION"))).size(12.0).color(MUTED));
            if let Some(tag) = v.update {
                if ui.link(format!("È disponibile la versione {tag}: scaricala")).clicked() {
                    v.actions.push(UiAction::OpenUpdate);
                }
            }

            section(ui, "Il tuo nome");
            ui.label(RichText::new("È quello che vedono i colleghi negli avvisi.").size(12.0).color(MUTED));
            ui.add(egui::TextEdit::singleline(&mut v.cfg.name).hint_text("es. Emanuele").desired_width(f32::INFINITY));

            section(ui, "Riconoscimento delle call");
            ui.radio_value(&mut v.cfg.detection, Detection::Auto, "Automatico: avvisa l'ufficio quando entro in call");
            ui.radio_value(&mut v.cfg.detection, Detection::Ask, "Chiedi conferma prima di avvisare");
            ui.radio_value(&mut v.cfg.detection, Detection::Off, "Disattivato: solo dal menu");
            ui.checkbox(&mut v.cfg.detect_unknown, "Considera call anche le app non riconosciute");

            section(ui, "Avvisi delle call dei colleghi");
            ui.radio_value(&mut v.cfg.alert_style, AlertStyle::Full, "Bordo rosso e riquadro (come Call Alert)");
            ui.radio_value(&mut v.cfg.alert_style, AlertStyle::Panel, "Solo il riquadro");
            ui.radio_value(&mut v.cfg.alert_style, AlertStyle::Tray, "Solo l'icona nella barra");
            ui.horizontal(|ui| {
                ui.label("Volume del suono");
                ui.add(egui::Slider::new(&mut v.cfg.volume, 0.0..=1.0).show_value(false));
            });

            section(ui, "Microfono in uso adesso");
            ui.label(RichText::new("Utile per capire cosa viene riconosciuto sul tuo computer.").size(12.0).color(MUTED));
            if v.mic.is_empty() {
                ui.label(RichText::new("Nessuna app sta usando il microfono").italics().color(MUTED));
            }
            for m in v.mic {
                ui.horizontal(|ui| {
                    let ignored = v.cfg.ignored_apps.contains(&m.id);
                    let status = if ignored {
                        "ignorata"
                    } else if m.known_call_app {
                        "app di call"
                    } else {
                        "non riconosciuta"
                    };
                    ui.label(RichText::new(&m.label).strong().color(TEXT));
                    ui.label(RichText::new(status).size(12.0).color(MUTED));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ignored {
                            if ui.small_button("Non ignorare").clicked() {
                                v.actions.push(UiAction::Unignore(m.id.clone()));
                            }
                        } else if ui.small_button("Ignora").clicked() {
                            v.actions.push(UiAction::Ignore(m.id.clone()));
                        }
                    });
                });
            }
            if !v.cfg.ignored_apps.is_empty() {
                ui.collapsing(format!("App ignorate ({})", v.cfg.ignored_apps.len()), |ui| {
                    for id in v.cfg.ignored_apps.clone() {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&id).size(11.0).color(MUTED));
                            if ui.small_button("Rimuovi").clicked() {
                                v.actions.push(UiAction::Unignore(id.clone()));
                            }
                        });
                    }
                });
            }

            section(ui, "Colleghi in rete");
            if v.peers.is_empty() {
                ui.label(RichText::new("Nessun collega trovato per ora").italics().color(MUTED));
            } else {
                ui.label(RichText::new(v.peers.join(", ")).color(TEXT));
            }
        });
    });
    if *v.cfg != before {
        v.actions.push(UiAction::ConfigChanged);
    }
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
