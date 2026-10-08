//! Sfondi animati: una finestra per schermo dietro alle icone, ritmo dei fotogrammi,
//! le tre modalità multimonitor.

use crate::config::{log, Config, Mode};
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
}

pub struct Wallpapers {
    screens: HashMap<WindowId, Screen>,
    pub monitors: Vec<MonitorInfo>,
    monitor_sig: String,
    start: Instant,
    /// Momento dell'ultimo cambio di sfondo: fa ripartire l'animazione di apertura.
    changed: Instant,
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
        Self { screens: HashMap::new(), monitors: Vec::new(), monitor_sig: String::new(), start: now, changed: now }
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
        let mut created: Vec<(Arc<Window>, wgpu::Surface<'static>, String, [f32; 2], [f32; 2], winit::dpi::PhysicalSize<u32>)> = Vec::new();
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
            created.push((window, surface, wallpaper, offset, canvas, size));
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
        g.retain_pipelines(&in_use);
        for (window, surface, wallpaper, offset, canvas, size) in created {
            let id = window.id();
            let scale = cfg.quality.scale(size.height);
            let target = g.make_target(window, surface, scale);
            self.screens.insert(id, Screen { target, size, wallpaper, offset, canvas, occluded: false });
        }
        self.changed = Instant::now();
        log(format!("Sfondi: {} schermi, modalità {:?}", self.screens.len(), cfg.mode));
    }

    pub fn clear(&mut self) {
        self.screens.clear();
    }

    pub fn render(&mut self, gpu: &mut Gpu, cfg: &Config, catalog: &[Wallpaper]) {
        if self.screens.is_empty() {
            return;
        }
        // Il tempo riparte ogni 6 ore per non perdere precisione nei calcoli in virgola mobile.
        let time = (self.start.elapsed().as_secs_f64() % 21_600.0) as f32;
        let since = self.changed.elapsed().as_secs_f32().min(1_000.0);
        let logo = if cfg.show_logo { 1.0 } else { 0.0 };
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
            };
            let w = catalog.iter().find(|w| w.id == screen.wallpaper).unwrap_or(&catalog[0]);
            gpu.draw(&mut screen.target, w, params);
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
