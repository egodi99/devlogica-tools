//! Sfondi animati: una finestra per schermo dietro alle icone, ritmo dei fotogrammi,
//! le tre modalità multimonitor.

use crate::config::{log, Config, Mode};
use crate::indicators::{dock_place, DockInfo, GRect};
use crate::overlay::{Frame, Overlay};
use crate::sysstats::Stats;
use crate::platform;
use crate::render::{self, Gpu, Params, Target};
use crate::tray::MonitorInfo;
use crate::wallpapers::Wallpaper;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use winit::dpi::{Position, Size};
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::monitor::MonitorHandle;
use winit::window::{Window, WindowId};

struct Screen {
    target: Target,
    /// Dimensione dello schermo in pixel fisici: la finestra deve restare esattamente così.
    size: winit::dpi::PhysicalSize<u32>,
    wallpaper: String,
    offset: [f32; 2],
    canvas: [f32; 2],
    occluded: bool,
    /// Posizione e dimensione dello schermo in punti globali (per trovare lo schermo del Dock).
    global: GRect,
    /// Angolo dello schermo in pixel fisici del desktop (Windows: coordinate del puntatore).
    phys: [i32; 2],
}

/// Dati degli indicatori, aggiornati dall'app ogni 2 secondi (solo macOS).
#[derive(Clone, Copy, Debug)]
pub struct IndicatorState {
    pub stats: Stats,
    pub hm: (u32, u32),
    pub dock: Option<DockInfo>,
}

pub struct Wallpapers {
    screens: HashMap<WindowId, Screen>,
    pub monitors: Vec<MonitorInfo>,
    monitor_sig: String,
    start: Instant,
    /// Momento dell'ultimo cambio di sfondo: fa ripartire l'animazione di apertura.
    changed: Instant,
    /// Disegno degli indicatori, uno per formato di superficie; creati solo quando servono.
    overlays: HashMap<wgpu::TextureFormat, Overlay>,
    /// Cruscotto: statistiche e lancette, solo se uno schermo lo mostra.
    dash: Option<crate::dash::DashRuntime>,
}

pub fn monitor_key(m: &MonitorHandle) -> String {
    let s = m.size();
    format!("{}|{}x{}", m.name().unwrap_or_else(|| "Schermo".into()), s.width, s.height)
}

pub fn monitor_list(el: &ActiveEventLoop) -> Vec<MonitorHandle> {
    let mut v: Vec<MonitorHandle> = el.available_monitors().collect();
    v.sort_by_key(|m| (m.position().x, m.position().y));
    v
}

pub fn signature(mons: &[MonitorHandle]) -> String {
    mons.iter()
        .map(|m| format!("{}@{},{}x{}", monitor_key(m), m.position().x, m.position().y, m.scale_factor()))
        .collect::<Vec<_>>()
        .join(";")
}

fn create_window(el: &ActiveEventLoop, m: &MonitorHandle) -> Option<Arc<Window>> {
    // Su macOS lo spazio delle coordinate è in punti: con schermi a scala diversa
    // (Retina + monitor esterno) bisogna convertire con la scala di *quello* schermo.
    #[cfg(target_os = "macos")]
    let (pos, size): (Position, Size) = {
        let s = m.scale_factor();
        (m.position().to_logical::<f64>(s).into(), m.size().to_logical::<f64>(s).into())
    };
    #[cfg(not(target_os = "macos"))]
    let (pos, size): (Position, Size) = (m.position().into(), m.size().into());

    #[allow(unused_mut)]
    let mut attrs = Window::default_attributes()
        .with_title("DevLogica Tools – sfondo")
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_position(pos)
        .with_inner_size(size);
    #[cfg(windows)]
    {
        use winit::platform::windows::WindowAttributesExtWindows;
        attrs = attrs.with_skip_taskbar(true);
    }
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attrs = attrs.with_has_shadow(false);
    }
    match el.create_window(attrs) {
        Ok(w) => Some(Arc::new(w)),
        Err(e) => {
            log(format!("Finestra non creata: {e}"));
            None
        }
    }
}

impl Wallpapers {
    pub fn new() -> Self {
        let now = Instant::now();
        Self { screens: HashMap::new(), monitors: Vec::new(), monitor_sig: String::new(), start: now, changed: now, overlays: HashMap::new(), dash: None }
    }

    pub fn owns(&self, id: WindowId) -> bool {
        self.screens.contains_key(&id)
    }

    pub fn monitors_changed(&self, el: &ActiveEventLoop) -> bool {
        signature(&monitor_list(el)) != self.monitor_sig || (!self.screens.is_empty() && platform::desktop_lost())
    }

    /// (Ri)crea le finestre degli sfondi. Crea anche la GPU, se non esiste ancora.
    pub fn rebuild(&mut self, el: &ActiveEventLoop, gpu: &mut Option<Gpu>, cfg: &Config) {
        self.screens.clear();
        if !cfg.indicators.enabled {
            self.overlays.clear();
        }
        let mons = monitor_list(el);
        self.monitor_sig = signature(&mons);
        for m in &mons {
            log(format!("Schermo trovato: {} pos {:?} dim {:?} scala {}", monitor_key(m), m.position(), m.size(), m.scale_factor()));
        }
        self.monitors = mons
            .iter()
            .map(|m| {
                let s = m.size();
                MonitorInfo { key: monitor_key(m), label: format!("{} ({}×{})", m.name().unwrap_or_default(), s.width, s.height) }
            })
            .collect();
        if mons.is_empty() || !cfg.wallpaper_enabled {
            self.dash = None;
            if gpu.is_none() {
                match Gpu::new(render::new_instance(), None) {
                    Ok(g) => *gpu = Some(g),
                    Err(e) => log(e),
                }
            }
            if let Some(g) = gpu.as_mut() {
                g.retain_pipelines(&[]);
            }
            return;
        }

        // Rettangolo che contiene tutti gli schermi (serve alla modalità estesa).
        let (mut x0, mut y0, mut x1, mut y1) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for m in &mons {
            let (p, s) = (m.position(), m.size());
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x + s.width as i32);
            y1 = y1.max(p.y + s.height as i32);
        }

        let instance = gpu.as_ref().map(|g| g.instance.clone()).unwrap_or_else(render::new_instance);
        let mut created: Vec<(Arc<Window>, wgpu::Surface<'static>, String, [f32; 2], [f32; 2], winit::dpi::PhysicalSize<u32>, GRect, [i32; 2])> = Vec::new();
        for m in &mons {
            let (pos, size) = (m.position(), m.size());
            let Some(window) = create_window(el, m) else { continue };
            if !platform::attach(&window, pos, size) {
                log(format!("Aggancio al desktop non riuscito per {}", monitor_key(m)));
            }
            window.set_visible(true);
            let surface = match instance.create_surface(window.clone()) {
                Ok(s) => s,
                Err(e) => {
                    log(format!("Superficie non creata: {e}"));
                    continue;
                }
            };
            let key = monitor_key(m);
            let wallpaper = match cfg.mode {
                Mode::PerMonitor => cfg.per_monitor.get(&key).cloned().unwrap_or_else(|| cfg.wallpaper.clone()),
                _ => cfg.wallpaper.clone(),
            };
            let (offset, canvas) = match cfg.mode {
                Mode::Span => ([(pos.x - x0) as f32, (pos.y - y0) as f32], [(x1 - x0) as f32, (y1 - y0) as f32]),
                _ => ([0.0, 0.0], [size.width as f32, size.height as f32]),
            };
            let sf = m.scale_factor();
            let (lp, ls) = (pos.to_logical::<f64>(sf), size.to_logical::<f64>(sf));
            let global = GRect { x: lp.x, y: lp.y, w: ls.width, h: ls.height };
            created.push((window, surface, wallpaper, offset, canvas, size, global, [pos.x, pos.y]));
        }

        if gpu.is_none() {
            match Gpu::new(instance, created.first().map(|c| &c.1)) {
                Ok(g) => *gpu = Some(g),
                Err(e) => {
                    log(e);
                    return;
                }
            }
        }
        let g = gpu.as_mut().unwrap();
        let in_use: Vec<String> = created.iter().map(|c| c.2.clone()).collect();
        if in_use.iter().any(|w| w == crate::dash::ID) {
            self.dash.get_or_insert_with(crate::dash::DashRuntime::new);
        } else {
            self.dash = None;
        }
        g.retain_pipelines(&in_use);
        for (window, surface, wallpaper, offset, canvas, size, global, phys) in created {
            let id = window.id();
            let scale = crate::indicators::indicator_scale(
                cfg.quality.scale(size.height),
                window.scale_factor() as f32,
                cfg!(target_os = "macos") && cfg.indicators.enabled,
            );
            let target = g.make_target(window, surface, scale);
            self.screens.insert(id, Screen { target, size, wallpaper, offset, canvas, occluded: false, global, phys });
        }
        self.changed = Instant::now();
        log(format!("Sfondi: {} schermi, modalità {:?}", self.screens.len(), cfg.mode));
    }

    /// Vero se almeno uno schermo mostra il cruscotto.
    pub fn dash_active(&self) -> bool {
        self.dash.is_some() && self.screens.values().any(|s| s.wallpaper == crate::dash::ID)
    }

    /// Tasto del cruscotto sotto il puntatore. Le coordinate sono quelle di `platform::pointer`:
    /// pixel fisici su Windows, punti globali su macOS.
    pub fn dash_hit(&self, x: f64, y: f64) -> Option<crate::dash::DashButton> {
        self.screens.values().filter(|s| s.wallpaper == crate::dash::ID).find_map(|s| {
            let (lx, ly) = if cfg!(target_os = "macos") {
                let k = s.target.window.scale_factor();
                ((x - s.global.x) * k, (y - s.global.y) * k)
            } else {
                (x - s.phys[0] as f64, y - s.phys[1] as f64)
            };
            if lx < 0.0 || ly < 0.0 || lx >= s.size.width as f64 || ly >= s.size.height as f64 {
                return None;
            }
            crate::dash::hit(s.canvas, [lx as f32 + s.offset[0], ly as f32 + s.offset[1]])
        })
    }

    pub fn dash_press(&mut self, b: crate::dash::DashButton) {
        if let Some(d) = self.dash.as_mut() {
            d.press(b);
        }
    }

    pub fn clear(&mut self) {
        self.screens.clear();
        self.dash = None;
    }

    pub fn render(&mut self, gpu: &mut Gpu, cfg: &Config, catalog: &[Wallpaper], ind: Option<&IndicatorState>, office: crate::dash::DashOffice) {
        if self.screens.is_empty() {
            return;
        }
        // Il tempo riparte ogni 6 ore per non perdere precisione nei calcoli in virgola mobile.
        let time = (self.start.elapsed().as_secs_f64() % 21_600.0) as f32;
        let since = self.changed.elapsed().as_secs_f32().min(1_000.0);
        let logo = if cfg.show_logo { 1.0 } else { 0.0 };
        // Gli indicatori vanno solo sullo schermo con il Dock (senza informazioni: quello principale).
        let ind = ind.filter(|_| cfg.indicators.enabled);
        let near = |a: f64, b: f64| (a - b).abs() <= 1.0;
        let dock_screen = |g: &GRect| match ind.and_then(|i| i.dock) {
            Some(d) => near(g.x, d.screen.x) && near(g.y, d.screen.y),
            None => near(g.x, 0.0) && near(g.y, 0.0),
        };
        let opacity = { let t = ((since - 1.3) / (2.6 - 1.3)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
        let dash_stats = self.dash.as_mut().map(|d| d.step(since));
        let road = self.dash.as_ref().map(|d| d.road).unwrap_or(0.0);
        for screen in self.screens.values_mut() {
            if screen.occluded {
                continue;
            }
            let size = screen.target.window.inner_size();
            let params = Params {
                res: [size.width as f32, size.height as f32],
                offset: screen.offset,
                canvas: screen.canvas,
                time,
                since,
                logo_on: logo,
                scale: screen.target.scale,
                _pad: [0.0; 2],
                dash: [road, 0.0, 0.0, 0.0],
            };
            let w = catalog.iter().find(|w| w.id == screen.wallpaper).unwrap_or(&catalog[0]);
            if w.id == crate::dash::ID {
                if let (Some(rt), Some(stats)) = (self.dash.as_mut(), dash_stats) {
                    let frame = crate::dash::DashFrame {
                        size_px: [screen.target.config.width, screen.target.config.height],
                        scale: screen.target.scale,
                        offset: screen.offset,
                        canvas: screen.canvas,
                        stats,
                        office,
                        since,
                        pressed: rt.pressed(),
                    };
                    let format = screen.target.format;
                    let mut extra = |g: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView| rt.draw(g, enc, view, format, &frame);
                    gpu.draw(&mut screen.target, w, params, Some(&mut extra));
                    continue;
                }
            }
            match ind.filter(|_| dock_screen(&screen.global)) {
                Some(state) => {
                    let format = screen.target.format;
                    let overlay = self.overlays.entry(format).or_insert_with(|| Overlay::new(gpu, format));
                    let dock = match state.dock {
                        Some(d) => dock_place(d.screen, d.dock, d.at_bottom),
                        None => crate::indicators::DockPlace::Unknown,
                    };
                    let frame = Frame { ind: &cfg.indicators, stats: state.stats, hm: state.hm, dock, opacity };
                    let size_px = [screen.target.config.width, screen.target.config.height];
                    let ppp = (screen.target.window.scale_factor() as f32) * screen.target.scale;
                    let mut extra = |g: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView| overlay.draw(g, enc, view, size_px, ppp, &frame);
                    gpu.draw(&mut screen.target, w, params, Some(&mut extra));
                }
                None => gpu.draw(&mut screen.target, w, params, None),
            }
        }
    }

    pub fn window_event(&mut self, gpu: Option<&Gpu>, cfg: &Config, id: WindowId, event: WindowEvent) {
        let Some(s) = self.screens.get_mut(&id) else { return };
        match event {
            WindowEvent::Occluded(o) => s.occluded = o,
            // Schermi con scala diversa (es. portatile al 150% + esterno al 100%): Windows
            // proporrebbe di ridimensionare la finestra quando cambia schermo. Deve invece
            // restare grande esattamente quanto lo schermo.
            WindowEvent::ScaleFactorChanged { mut inner_size_writer, .. } => {
                let _ = inner_size_writer.request_inner_size(s.size);
            }
            WindowEvent::Resized(size) => {
                if let Some(gpu) = gpu {
                    gpu.resize(&mut s.target, size.width, size.height);
                }
                if cfg.mode != Mode::Span {
                    s.canvas = [size.width as f32, size.height as f32];
                }
                if size != s.size {
                    log(format!("Sfondo ridimensionato a {}×{} (atteso {}×{}): lo riporto alla misura dello schermo", size.width, size.height, s.size.width, s.size.height));
                    let _ = s.target.window.request_inner_size(s.size);
                }
            }
            _ => {}
        }
    }
}
