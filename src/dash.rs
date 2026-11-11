//! Sfondo "Cruscotto": la plancia di un camion svedese anni '90 di notte, con strumenti
//! che si muovono davvero. Lo shader (shaders/cruscotto.wgsl) disegna cabina, strada, plastica,
//! radica, quadranti e volante; sopra, nello stesso fotogramma, egui disegna scale, numeri,
//! lancette, spie, display e interruttori con i valori del PC.
//!
//! Tutte le posizioni sono in uno spazio di progetto 1920×1080, adattato allo schermo
//! (ancorato in basso, centrato). Le posizioni dei quadranti stanno solo qui: allo shader
//! arrivano come costanti generate (`wgsl_consts`).

use crate::render::{Gpu, Params};
use crate::theme;
use crate::wallpapers::Wallpaper;
use egui::{Align2, Color32, FontFamily, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use std::f32::consts::PI;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const ID: &str = "cruscotto";
pub const DW: f32 = 1920.0;
pub const DH: f32 = 1080.0;

// ---------------------------------------------------------------- dati

/// Letture del PC per il cruscotto. `None` = non disponibile (o spento, per WSL e Docker).
#[derive(Clone, Copy, Debug, Default)]
pub struct DashStats {
    pub ready: bool,
    pub cpu: f32,
    pub ram: f32,
    pub ram_total_gb: f32,
    pub gpu: Option<f32>,
    pub disk: Option<f32>,
    pub net_mbit: Option<f32>,
    pub wsl_gb: Option<f32>,
    pub docker: Option<u32>,
    pub disk_c: Option<f32>,
    pub uptime_h: f32,
    pub battery: Option<(f32, bool)>,
}

/// Stato dell'ufficio mostrato dalle spie.
#[derive(Clone, Copy, Debug, Default)]
pub struct DashOffice {
    pub call: bool,
    pub coffee: bool,
    pub auto_detect: bool,
}

/// Valori di esempio (anteprime e miniature).
pub fn demo_stats() -> DashStats {
    DashStats {
        ready: true,
        cpu: 38.0,
        ram: 71.0,
        ram_total_gb: 32.0,
        gpu: Some(22.0),
        disk: Some(12.0),
        net_mbit: Some(54.0),
        wsl_gb: Some(6.4),
        docker: Some(4),
        disk_c: Some(64.0),
        uptime_h: 127.4,
        battery: None,
    }
}

/// Conta i container nella risposta HTTP di `GET /containers/json` del demone Docker.
pub fn count_containers(resp: &[u8]) -> Option<u32> {
    let text = std::str::from_utf8(resp).ok()?;
    let (head, body) = text.split_once("\r\n\r\n")?;
    if !head.starts_with("HTTP/1.") || !head.split_whitespace().nth(1).is_some_and(|c| c == "200") {
        return None;
    }
    let list: Vec<serde_json::Value> = serde_json::from_str(body.trim()).ok()?;
    Some(list.len() as u32)
}

/// Occupazione della GPU dalle istanze "pid_…_luid_…_phys_0_eng_3_engtype_3D": somma per
/// motore (tutto da "luid" in poi), poi il motore più occupato.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn gpu_from_engines(items: &[(String, f64)]) -> f64 {
    let mut per: std::collections::HashMap<&str, f64> = std::collections::HashMap::new();
    for (name, v) in items {
        let key = name.find("luid").map(|i| &name[i..]).unwrap_or(name.as_str());
        *per.entry(key).or_default() += v;
    }
    per.values().copied().fold(0.0, f64::max)
}

#[cfg(unix)]
fn docker_running() -> Option<u32> {
    use std::io::{Read, Write};
    let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
    let paths = [Some(std::path::PathBuf::from("/var/run/docker.sock")), home.map(|h| h.join(".docker/run/docker.sock"))];
    for p in paths.into_iter().flatten() {
        let Ok(mut s) = std::os::unix::net::UnixStream::connect(&p) else { continue };
        let _ = s.set_read_timeout(Some(Duration::from_secs(3)));
        s.write_all(b"GET /containers/json HTTP/1.0\r\nHost: docker\r\n\r\n").ok()?;
        let mut out = Vec::new();
        let _ = s.read_to_end(&mut out);
        return count_containers(&out);
    }
    None
}

#[cfg(windows)]
fn docker_running() -> Option<u32> {
    crate::platform::probe_windows::docker_running()
}

/// Campiona le statistiche in un thread a parte (ogni secondo; Docker ogni 5 secondi).
/// Si ferma da solo quando il campionatore viene buttato.
pub struct DashSampler {
    latest: Arc<Mutex<DashStats>>,
    alive: Arc<()>,
}

impl DashSampler {
    pub fn start() -> Self {
        let latest = Arc::new(Mutex::new(DashStats::default()));
        let alive = Arc::new(());
        let docker = Arc::new(Mutex::new(None::<u32>));
        {
            let (docker, weak) = (docker.clone(), Arc::downgrade(&alive));
            let _ = std::thread::Builder::new().name("cruscotto docker".into()).spawn(move || {
                while weak.upgrade().is_some() {
                    let n = docker_running();
                    if let Ok(mut d) = docker.lock() {
                        *d = n;
                    }
                    std::thread::sleep(Duration::from_secs(5));
                }
            });
        }
        let (out, weak) = (latest.clone(), Arc::downgrade(&alive));
        let _ = std::thread::Builder::new().name("cruscotto".into()).spawn(move || {
            #[cfg(windows)]
            let mut probe = crate::platform::probe_windows::Probe::new();
            #[cfg(not(windows))]
            let mut prev: Option<[u64; 4]> = crate::platform::cpu_ticks();
            let started = Instant::now();
            std::thread::sleep(Duration::from_millis(400));
            while weak.upgrade().is_some() {
                #[cfg(windows)]
                let mut s = probe.sample();
                #[cfg(not(windows))]
                let mut s = {
                    let ticks = crate::platform::cpu_ticks();
                    let cpu = match (prev, ticks) {
                        (Some(p), Some(c)) => crate::sysstats::cpu_percent(p, c),
                        _ => 0.0,
                    };
                    prev = ticks;
                    let total = crate::platform::mem_total().unwrap_or(0);
                    let used = crate::platform::vm_pages().map(|(i, p, w, c, pg)| crate::sysstats::ram_used(i, p, w, c, pg)).unwrap_or(0);
                    DashStats {
                        ready: true,
                        cpu,
                        ram: if total > 0 { used as f32 / total as f32 * 100.0 } else { 0.0 },
                        ram_total_gb: total as f32 / 1_073_741_824.0,
                        uptime_h: started.elapsed().as_secs_f32() / 3600.0,
                        ..Default::default()
                    }
                };
                let _ = started;
                s.docker = docker.lock().ok().and_then(|d| *d);
                if let Ok(mut l) = out.lock() {
                    *l = s;
                }
                std::thread::sleep(Duration::from_secs(1));
            }
        });
        Self { latest, alive }
    }

    pub fn latest(&self) -> DashStats {
        let _ = &self.alive;
        self.latest.lock().map(|s| *s).unwrap_or_default()
    }
}

// ---------------------------------------------------------------- disposizione

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Dial {
    Big,
    Small,
    Clock,
}

/// Quadrante: centro e raggio nello spazio di progetto.
#[derive(Clone, Copy, Debug)]
pub struct Gauge {
    pub x: f32,
    pub y: f32,
    pub r: f32,
    pub dial: Dial,
}

const fn g(x: f32, y: f32, r: f32, dial: Dial) -> Gauge {
    Gauge { x, y, r, dial }
}

/// Contagiri (CPU), tachimetro (rete), sei strumenti piccoli, orologio.
pub const GAUGES: [Gauge; 9] = [
    g(806.0, 735.0, 138.0, Dial::Big),
    g(1094.0, 735.0, 138.0, Dial::Big),
    g(474.0, 640.0, 48.0, Dial::Small),
    g(600.0, 640.0, 48.0, Dial::Small),
    g(474.0, 744.0, 48.0, Dial::Small),
    g(600.0, 744.0, 48.0, Dial::Small),
    g(474.0, 848.0, 48.0, Dial::Small),
    g(600.0, 848.0, 48.0, Dial::Small),
    g(1752.0, 626.0, 56.0, Dial::Clock),
];

/// Spie nel quadro: 2 colonne × 5 righe.
fn light_rect(i: usize) -> Rect {
    let (col, row) = (i % 2, i / 2);
    Rect::from_center_size(Pos2::new(1276.0 + 60.0 * col as f32, 616.0 + 56.0 * row as f32), Vec2::new(50.0, 42.0))
}

/// Interruttori a bilanciere nella console di destra: 2 righe × 6.
fn rocker_rect(i: usize) -> Rect {
    let (col, row) = (i % 6, i / 6);
    Rect::from_center_size(Pos2::new(1484.0 + 60.0 * col as f32, 766.0 + 98.0 * row as f32), Vec2::new(46.0, 64.0))
}

const LCD: Rect = Rect { min: Pos2 { x: 1470.0, y: 580.0 }, max: Pos2 { x: 1650.0, y: 672.0 } };

/// Costanti per lo shader: le stesse posizioni dei quadranti e degli interruttori.
pub fn wgsl_consts() -> String {
    let mut s = String::from("// ---- generato da dash.rs ----\n");
    s += &format!("const N_GAUGES: u32 = {}u;\n", GAUGES.len());
    s += "const GAUGES = array<vec4f, 9>(\n";
    for g in GAUGES {
        let kind = match g.dial {
            Dial::Big => 0.0,
            Dial::Small => 1.0,
            Dial::Clock => 2.0,
        };
        s += &format!("    vec4f({:.1}, {:.1}, {:.1}, {:.1}),\n", g.x, g.y, g.r, kind);
    }
    s += ");\n";
    let r = (0..12).map(rocker_rect).collect::<Vec<_>>();
    s += "const ROCKERS = array<vec4f, 12>(\n";
    for x in r {
        s += &format!("    vec4f({:.1}, {:.1}, {:.1}, {:.1}),\n", x.center().x, x.center().y, x.width() / 2.0, x.height() / 2.0);
    }
    s += ");\n";
    s += &format!("const LCD_BOX = vec4f({:.1}, {:.1}, {:.1}, {:.1});\n", LCD.center().x, LCD.center().y, LCD.width() / 2.0, LCD.height() / 2.0);
    s
}

// ---------------------------------------------------------------- stato animato

/// Valori mostrati dalle lancette: inseguono quelli veri con un po' d'inerzia, come
/// strumenti meccanici. All'avvio fanno il giro di prova fino a fondo scala.
pub struct DashRuntime {
    pub sampler: DashSampler,
    shown: [f32; 9],
    last: Instant,
    /// Distanza percorsa dalla strada nello shader (cresce con il tachimetro).
    pub road: f32,
    overlays: std::collections::HashMap<wgpu::TextureFormat, DashOverlay>,
}

/// Fondo scala degli strumenti, nello stesso ordine di `GAUGES` (l'orologio non conta).
fn ranges(s: &DashStats) -> [f32; 8] {
    let wsl_max = (s.ram_total_gb * 0.5).max(4.0).round();
    [100.0, 125.0, 100.0, 100.0, wsl_max, 10.0, 100.0, 100.0]
}

fn targets(s: &DashStats) -> [f32; 8] {
    [
        s.cpu,
        s.net_mbit.unwrap_or(0.0),
        s.ram,
        s.gpu.unwrap_or(0.0),
        s.wsl_gb.unwrap_or(0.0),
        s.docker.unwrap_or(0) as f32,
        s.disk.unwrap_or(0.0),
        s.disk_c.unwrap_or(0.0),
    ]
}

impl DashRuntime {
    pub fn new() -> Self {
        Self { sampler: DashSampler::start(), shown: [0.0; 9], last: Instant::now(), road: 0.0, overlays: Default::default() }
    }

    /// Avanza le lancette e la strada; `since` = secondi dall'ultimo cambio di sfondo.
    pub fn step(&mut self, since: f32) -> DashStats {
        let s = self.sampler.latest();
        let dt = self.last.elapsed().as_secs_f32().min(0.5);
        self.last = Instant::now();
        let k = 1.0 - (-dt * 3.2).exp();
        let (t, r) = (targets(&s), ranges(&s));
        for i in 0..8 {
            // giro di prova all'avvio: su fino a fondo scala e giù
            let sweep = sweep_k(since);
            let target = if sweep >= 0.0 { r[i] * sweep } else { t[i] };
            let kk = if sweep >= 0.0 { 1.0 } else { k };
            self.shown[i] += (target.min(r[i] * 1.02) - self.shown[i]) * kk;
        }
        // la strada scorre alla "velocità" del tachimetro (minimo un passo d'uomo)
        let kmh = self.shown[1].max(0.0);
        self.road = (self.road + dt * (kmh * 0.045 + 0.08)) % 1000.0;
        s
    }

    pub fn draw(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, format: wgpu::TextureFormat, frame: &DashFrame) {
        let ov = self.overlays.entry(format).or_insert_with(|| DashOverlay::new(gpu, format));
        ov.draw(gpu, enc, view, frame, &self.shown);
    }
}

/// 0..1..0 nei primi 1,8 secondi dopo il cambio di sfondo, poi -1.
fn sweep_k(since: f32) -> f32 {
    let (t0, t1) = (0.5, 2.3);
    if since < t0 {
        return 0.0;
    }
    if since > t1 {
        return -1.0;
    }
    let x = (since - t0) / (t1 - t0);
    (x * PI).sin().powf(0.8)
}

// ---------------------------------------------------------------- disegno

/// Un fotogramma: dove sta la superficie dentro la tela e cosa mostrare.
pub struct DashFrame {
    pub size_px: [u32; 2],
    /// Pixel della superficie per pixel della finestra (risoluzione ridotta).
    pub scale: f32,
    pub offset: [f32; 2],
    pub canvas: [f32; 2],
    pub stats: DashStats,
    pub office: DashOffice,
    pub since: f32,
}

pub struct DashOverlay {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
}

fn din() -> FontFamily {
    FontFamily::Name("din".into())
}
fn icons() -> FontFamily {
    FontFamily::Name("icons".into())
}

impl DashOverlay {
    pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self {
        let ctx = egui::Context::default();
        theme::install_fonts(&ctx);
        // Bahnschrift: il carattere DIN di Windows, quello giusto per un cruscotto
        let mut defs = egui::FontDefinitions::default();
        let base = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
        let mut din_list = base.clone();
        // ripiego incluso, dove Bahnschrift non c'è (macOS): Barlow, della stessa famiglia DIN
        defs.font_data.insert("barlow".into(), Arc::new(egui::FontData::from_static(theme::BARLOW)));
        din_list.insert(0, "barlow".into());
        if let Some(b) = theme::system_font(&["bahnschrift.ttf"]) {
            defs.font_data.insert("bahnschrift".into(), b);
            din_list.insert(0, "bahnschrift".into());
        }
        let ic_list: Vec<String> = theme::add_icon_fonts(&mut defs).into_iter().chain(base).collect();
        defs.families.insert(din(), din_list);
        defs.families.insert(icons(), ic_list);
        ctx.set_fonts(defs);
        let renderer = egui_wgpu::Renderer::new(&gpu.device, format, egui_wgpu::RendererOptions::default());
        Self { ctx, renderer }
    }

    pub fn draw(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, f: &DashFrame, shown: &[f32; 9]) {
        let screen = Vec2::new(f.size_px[0] as f32, f.size_px[1] as f32);
        let raw = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, screen)),
            viewports: std::iter::once((egui::ViewportId::ROOT, egui::ViewportInfo { native_pixels_per_point: Some(1.0), ..Default::default() })).collect(),
            ..Default::default()
        };
        let mut out = self.ctx.run_ui(raw, |ui| {
            let painter = ui.painter().clone();
            paint(&painter, f, shown);
        });
        for (id, deltas) in &out.textures_delta.set {
            for d in deltas.iter() {
                self.renderer.update_texture(&gpu.device, &gpu.queue, *id, d);
            }
        }
        let free: Vec<egui::TextureId> = out.textures_delta.free.iter().copied().collect();
        out.textures_delta.clear();
        let prims = self.ctx.tessellate(std::mem::take(&mut out.shapes), 1.0);
        let sd = egui_wgpu::ScreenDescriptor { size_in_pixels: f.size_px, pixels_per_point: 1.0 };
        let extra = self.renderer.update_buffers(&gpu.device, &gpu.queue, enc, &prims, &sd);
        if !extra.is_empty() {
            gpu.queue.submit(extra);
        }
        {
            let pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cruscotto"),
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

/// Dallo spazio di progetto ai pixel della superficie.
struct Map {
    k: f32,
    o: Vec2,
}

impl Map {
    fn new(f: &DashFrame) -> Self {
        let (cw, ch) = (f.canvas[0], f.canvas[1]);
        let k = (cw / DW).min(ch / DH);
        let o = Vec2::new((cw - DW * k) / 2.0, ch - DH * k);
        // la superficie copre la finestra a partire da `offset` nella tela, ridotta di `scale`
        Map { k: k * f.scale, o: (o - Vec2::new(f.offset[0], f.offset[1])) * f.scale }
    }
    fn p(&self, x: f32, y: f32) -> Pos2 {
        Pos2::new(self.o.x + x * self.k, self.o.y + y * self.k)
    }
    fn l(&self, v: f32) -> f32 {
        v * self.k
    }
    fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.p(r.min.x, r.min.y), self.p(r.max.x, r.max.y))
    }
}

const NEEDLE: Color32 = Color32::from_rgb(255, 122, 28);
const SCALE: Color32 = Color32::from_rgb(236, 238, 230);
const SCALE_DIM: Color32 = Color32::from_rgb(150, 152, 148);
const ECO: Color32 = Color32::from_rgb(52, 178, 92);
const ZONE: Color32 = Color32::from_rgb(232, 52, 40);

fn polar(c: Pos2, r: f32, a: f32) -> Pos2 {
    Pos2::new(c.x + r * a.cos(), c.y + r * a.sin())
}

fn arc(p: &egui::Painter, c: Pos2, r: f32, a0: f32, a1: f32, w: f32, color: Color32) {
    let n = (((a1 - a0).abs() * r / 4.0) as usize).clamp(6, 160);
    let pts: Vec<Pos2> = (0..=n).map(|i| polar(c, r, a0 + (a1 - a0) * i as f32 / n as f32)).collect();
    p.add(Shape::line(pts, Stroke::new(w, color)));
}

struct Scale {
    max: f32,
    /// Ogni quanto un numero e ogni quanto una tacca piccola.
    label_every: f32,
    minor: f32,
    /// Fattore del numero scritto (es. 0.1 per scrivere 0..10 su 0..100).
    label_k: f32,
    eco: Option<(f32, f32)>,
    red: Option<f32>,
}

fn needle(p: &egui::Painter, c: Pos2, a: f32, len: f32, tail: f32, w: f32) {
    let dir = Vec2::new(a.cos(), a.sin());
    let n = Vec2::new(-dir.y, dir.x);
    let tip = c + dir * len;
    let back = c - dir * tail;
    let body = vec![back + n * w * 0.55, c + n * w * 0.5, tip + n * w * 0.12, tip - n * w * 0.12, c - n * w * 0.5, back - n * w * 0.55];
    // alone luminoso, poi la lancetta
    let glow: Vec<Pos2> = body.iter().map(|q| c + (*q - c) * 1.0 + (*q - c).normalized() * w * 0.6).collect();
    p.add(Shape::convex_polygon(glow, Color32::from_rgba_unmultiplied(255, 110, 20, 38), Stroke::NONE));
    p.add(Shape::convex_polygon(body, NEEDLE, Stroke::NONE));
}

fn hub(p: &egui::Painter, c: Pos2, r: f32) {
    p.circle_filled(c + Vec2::new(0.0, r * 0.15), r * 1.05, Color32::from_rgba_unmultiplied(0, 0, 0, 120));
    p.circle_filled(c, r, Color32::from_rgb(26, 27, 30));
    p.circle_stroke(c, r * 0.92, Stroke::new((r * 0.12).max(1.0), Color32::from_rgb(58, 60, 64)));
    p.circle_filled(c - Vec2::new(r * 0.25, r * 0.3), r * 0.25, Color32::from_rgba_unmultiplied(255, 255, 255, 18));
}

fn fmt_num(v: f32) -> String {
    if (v - v.round()).abs() < 0.01 { format!("{}", v.round() as i32) } else { format!("{v:.1}") }
}

#[allow(clippy::too_many_arguments)]
fn big_gauge(p: &egui::Painter, m: &Map, g: Gauge, s: &Scale, value: f32, title: &str, unit: &str, lcd: &str) {
    let c = m.p(g.x, g.y);
    let r = m.l(g.r);
    let (a0, sweep) = (0.75 * PI, 1.5 * PI);
    let ang = |v: f32| a0 + sweep * (v / s.max).clamp(0.0, 1.02);
    // zone colorate
    if let Some((e0, e1)) = s.eco {
        arc(p, c, r * 0.845, ang(e0), ang(e1), r * 0.05, ECO.gamma_multiply(0.9));
    }
    if let Some(z) = s.red {
        arc(p, c, r * 0.845, ang(z), ang(s.max), r * 0.05, ZONE);
    }
    // tacche
    let mut v = 0.0;
    while v <= s.max + 0.001 {
        let major = (v / s.label_every - (v / s.label_every).round()).abs() < 0.001;
        let half = !major && (v / (s.label_every / 2.0) - (v / (s.label_every / 2.0)).round()).abs() < 0.001;
        let (r0, w) = if major { (r * 0.76, r * 0.022) } else if half { (r * 0.82, r * 0.016) } else { (r * 0.86, r * 0.011) };
        let a = ang(v);
        let col = if s.red.is_some_and(|z| v >= z - 0.001) { ZONE } else { SCALE };
        p.line_segment([polar(c, r0, a), polar(c, r * 0.92, a)], Stroke::new(w.max(1.0), col));
        if major {
            let txt = fmt_num(v * s.label_k);
            p.text(polar(c, r * 0.6, a), Align2::CENTER_CENTER, txt, FontId::new(r * 0.17, din()), SCALE);
        }
        v += s.minor;
    }
    // scritte
    p.text(c + Vec2::new(0.0, -r * 0.3), Align2::CENTER_CENTER, "DEVLOGICA", FontId::new(r * 0.075, din()), SCALE_DIM);
    p.text(c + Vec2::new(0.0, r * 0.3), Align2::CENTER_CENTER, title, FontId::new(r * 0.105, din()), SCALE);
    p.text(c + Vec2::new(0.0, r * 0.43), Align2::CENTER_CENTER, unit, FontId::new(r * 0.075, din()), SCALE_DIM);
    // display in basso (contaore / valore)
    let lr = Rect::from_center_size(c + Vec2::new(0.0, r * 0.7), Vec2::new(r * 0.66, r * 0.17));
    p.rect_filled(lr, r * 0.02, Color32::from_rgb(10, 12, 10));
    p.rect_stroke(lr, r * 0.02, Stroke::new(1.0, Color32::from_rgb(54, 56, 58)), egui::StrokeKind::Outside);
    p.text(lr.center(), Align2::CENTER_CENTER, lcd, FontId::new(r * 0.115, din()), Color32::from_rgb(255, 160, 70));
    needle(p, c, ang(value), r * 0.9, r * 0.2, r * 0.07);
    hub(p, c, r * 0.11);
}

fn small_gauge(p: &egui::Painter, m: &Map, g: Gauge, max: f32, red: Option<f32>, value: f32, label: &str, reading: &str, off: bool) {
    let c = m.p(g.x, g.y + 8.0);
    let r = m.l(g.r);
    let (a0, sweep) = (1.17 * PI, 0.66 * PI);
    let ang = |v: f32| a0 + sweep * (v / max).clamp(0.0, 1.02);
    if let Some(z) = red {
        arc(p, c, r * 0.83, ang(z), ang(max), r * 0.07, ZONE);
    }
    for i in 0..=8 {
        let v = max * i as f32 / 8.0;
        let major = i % 4 == 0;
        let (r0, w) = if major { (r * 0.66, r * 0.05) } else { (r * 0.74, r * 0.03) };
        let col = if red.is_some_and(|z| v >= z - 0.001) { ZONE } else { SCALE };
        p.line_segment([polar(c, r0, ang(v)), polar(c, r * 0.9, ang(v))], Stroke::new(w.max(1.0), col));
    }
    let lab_col = if off { SCALE_DIM.gamma_multiply(0.6) } else { SCALE };
    p.text(c + Vec2::new(0.0, r * 0.3), Align2::CENTER_CENTER, label, FontId::new(r * 0.26, din()), lab_col);
    p.text(c + Vec2::new(0.0, r * 0.58), Align2::CENTER_CENTER, reading, FontId::new(r * 0.24, din()), if off { SCALE_DIM.gamma_multiply(0.6) } else { Color32::from_rgb(255, 160, 70) });
    needle(p, c, ang(value), r * 0.86, r * 0.16, r * 0.1);
    hub(p, c, r * 0.14);
}

fn clock(p: &egui::Painter, m: &Map, g: Gauge) {
    use chrono::Timelike;
    let c = m.p(g.x, g.y);
    let r = m.l(g.r);
    for i in 0..60 {
        let a = i as f32 / 60.0 * 2.0 * PI - PI / 2.0;
        let (r0, w) = if i % 15 == 0 { (r * 0.7, r * 0.06) } else if i % 5 == 0 { (r * 0.76, r * 0.04) } else { (r * 0.84, r * 0.015) };
        p.line_segment([polar(c, r0, a), polar(c, r * 0.9, a)], Stroke::new(w.max(1.0), SCALE));
    }
    p.text(c + Vec2::new(0.0, r * 0.32), Align2::CENTER_CENTER, "DEVLOGICA", FontId::new(r * 0.11, din()), SCALE_DIM);
    let now = chrono::Local::now();
    let (h, mi, s) = (now.hour() % 12, now.minute(), now.second());
    let sf = now.nanosecond() as f32 / 1e9;
    let ha = ((h as f32 + mi as f32 / 60.0) / 12.0) * 2.0 * PI - PI / 2.0;
    let ma = ((mi as f32 + s as f32 / 60.0) / 60.0) * 2.0 * PI - PI / 2.0;
    // lancetta dei secondi "a scatti" come gli orologi al quarzo, con un piccolo rimbalzo
    let tick = s as f32 + (sf * 14.0).min(1.0);
    let sa = tick / 60.0 * 2.0 * PI - PI / 2.0;
    let hand = |a: f32, len: f32, w: f32, col: Color32| {
        let d = Vec2::new(a.cos(), a.sin());
        p.line_segment([c - d * r * 0.1, c + d * len], Stroke::new(w, col));
    };
    hand(ha, r * 0.5, r * 0.08, SCALE);
    hand(ma, r * 0.78, r * 0.055, SCALE);
    hand(sa, r * 0.86, (r * 0.022).max(1.0), NEEDLE);
    hub(p, c, r * 0.09);
}

/// Spia: lente scura, accesa con alone del suo colore.
fn lamp(p: &egui::Painter, m: &Map, r: Rect, on: bool, color: Color32, glyph: &str, text: bool) {
    let rr = m.rect(r.shrink(3.0));
    let rad = m.l(6.0);
    p.rect_filled(rr, rad, Color32::from_rgb(12, 13, 15));
    if on {
        p.rect_filled(rr.expand(m.l(6.0)), rad * 2.0, color.gamma_multiply(0.10));
        p.rect_filled(rr.expand(m.l(2.0)), rad * 1.4, color.gamma_multiply(0.18));
        p.rect_filled(rr, rad, color.gamma_multiply(0.22));
    }
    p.rect_stroke(rr, rad, Stroke::new(1.0, Color32::from_rgb(38, 40, 44)), egui::StrokeKind::Outside);
    let fg = if on { color } else { Color32::from_rgb(52, 54, 58) };
    let font = if text { FontId::new(m.l(15.0), din()) } else { FontId::new(m.l(20.0), icons()) };
    p.text(rr.center(), Align2::CENTER_CENTER, glyph, font, fg);
}

/// Interruttore a bilanciere con il LED in alto.
fn rocker(p: &egui::Painter, m: &Map, r: Rect, led: Option<Color32>, glyph: &str, text: bool) {
    let rr = m.rect(r);
    let rad = m.l(5.0);
    p.rect_filled(rr.translate(Vec2::new(0.0, m.l(3.0))), rad, Color32::from_rgba_unmultiplied(0, 0, 0, 140));
    p.rect_filled(rr, rad, Color32::from_rgb(30, 31, 34));
    // metà superiore leggermente sporgente
    let top = Rect::from_min_max(rr.min, Pos2::new(rr.max.x, rr.center().y));
    p.rect_filled(top.shrink(m.l(2.0)), rad, Color32::from_rgb(40, 42, 46));
    p.line_segment([Pos2::new(rr.left() + rad, rr.top() + 1.0), Pos2::new(rr.right() - rad, rr.top() + 1.0)], Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 26)));
    let led_r = Rect::from_center_size(Pos2::new(rr.center().x, rr.top() + m.l(9.0)), Vec2::new(m.l(16.0), m.l(4.0)));
    match led {
        Some(col) => {
            p.rect_filled(led_r.expand(m.l(4.0)), m.l(4.0), col.gamma_multiply(0.18));
            p.rect_filled(led_r, m.l(1.5), col);
        }
        None => {
            p.rect_filled(led_r, m.l(1.5), Color32::from_rgb(22, 22, 24));
        }
    }
    let font = if text { FontId::new(m.l(13.0), din()) } else { FontId::new(m.l(17.0), icons()) };
    p.text(Pos2::new(rr.center().x, rr.top() + m.l(27.0)), Align2::CENTER_CENTER, glyph, font, Color32::from_rgb(196, 198, 192));
}

fn lcd(p: &egui::Painter, m: &Map, st: &DashStats) {
    let r = m.rect(LCD.shrink(6.0));
    let rad = m.l(3.0);
    p.rect_filled(r.expand(m.l(10.0)), rad * 3.0, Color32::from_rgba_unmultiplied(150, 210, 70, 14));
    p.rect_filled(r, rad, Color32::from_rgb(104, 140, 46));
    // gradiente della retroilluminazione: più chiaro al centro
    p.rect_filled(r.shrink2(Vec2::new(r.width() * 0.12, r.height() * 0.18)), rad, Color32::from_rgba_unmultiplied(170, 214, 92, 40));
    let ink = Color32::from_rgb(22, 34, 10);
    let small = FontId::new(m.l(14.0), din());
    let big = FontId::new(m.l(30.0), din());
    let pad = m.l(10.0);
    let (docker, wsl) = (
        st.docker.map(|n| n.to_string()).unwrap_or_else(|| "OFF".into()),
        st.wsl_gb.map(|g| format!("{g:.1}")).unwrap_or_else(|| "OFF".into()),
    );
    let row = |y: f32, label: &str, val: &str, unit: &str| {
        p.text(Pos2::new(r.left() + pad, y), Align2::LEFT_CENTER, label, small.clone(), ink);
        let ux = r.right() - pad;
        let uw = if unit.is_empty() { 0.0 } else { m.l(24.0) };
        p.text(Pos2::new(ux, y + m.l(4.0)), Align2::RIGHT_CENTER, unit, small.clone(), ink);
        p.text(Pos2::new(ux - uw, y), Align2::RIGHT_CENTER, val, big.clone(), ink);
    };
    let wsl_unit = if st.wsl_gb.is_some() { "GB" } else { "" };
    row(r.top() + r.height() * 0.28, "DOCKER", &docker, "");
    row(r.top() + r.height() * 0.74, "WSL", &wsl, wsl_unit);
}

/// Riflesso del vetro sul quadro strumenti.
fn glass(p: &egui::Painter, m: &Map) {
    let pts = vec![m.p(392.0, 572.0), m.p(980.0, 572.0), m.p(760.0, 700.0), m.p(392.0, 760.0)];
    p.add(Shape::convex_polygon(pts, Color32::from_rgba_unmultiplied(255, 255, 255, 6), Stroke::NONE));
    let pts = vec![m.p(1010.0, 572.0), m.p(1090.0, 572.0), m.p(860.0, 896.0), m.p(780.0, 896.0)];
    p.add(Shape::convex_polygon(pts, Color32::from_rgba_unmultiplied(255, 255, 255, 4), Stroke::NONE));
}

fn paint(p: &egui::Painter, f: &DashFrame, shown: &[f32; 9]) {
    let m = Map::new(f);
    let st = &f.stats;
    let ready = st.ready;
    let blink = ((f.since * 1.6) % 1.0) < 0.6;

    // ---- strumenti grandi ----
    let cpu_lcd = if ready { format!("{:>3.0} %", st.cpu) } else { "--- %".into() };
    big_gauge(p, &m, GAUGES[0], &Scale { max: 100.0, label_every: 10.0, minor: 2.0, label_k: 0.1, eco: Some((20.0, 60.0)), red: Some(85.0) }, shown[0], "CPU", "×10 %", &cpu_lcd);
    let up = format!("{:07.1} h", st.uptime_h.min(999_999.0));
    big_gauge(p, &m, GAUGES[1], &Scale { max: 125.0, label_every: 20.0, minor: 5.0, label_k: 1.0, eco: None, red: None }, shown[1], "RETE", "Mbit/s", &up);

    // ---- strumenti piccoli ----
    let r = ranges(st);
    let pct = |v: Option<f32>| v.map(|x| format!("{x:.0}%")).unwrap_or_else(|| "--".into());
    small_gauge(p, &m, GAUGES[2], r[2], Some(85.0), shown[2], "RAM", &pct(Some(st.ram)), false);
    small_gauge(p, &m, GAUGES[3], r[3], Some(90.0), shown[3], "GPU", &pct(st.gpu), st.gpu.is_none());
    small_gauge(p, &m, GAUGES[4], r[4], None, shown[4], "WSL", &st.wsl_gb.map(|g| format!("{g:.1} GB")).unwrap_or_else(|| "OFF".into()), st.wsl_gb.is_none());
    small_gauge(p, &m, GAUGES[5], r[5], None, shown[5], "DOCKER", &st.docker.map(|n| n.to_string()).unwrap_or_else(|| "OFF".into()), st.docker.is_none());
    small_gauge(p, &m, GAUGES[6], r[6], Some(90.0), shown[6], "DISCO", &pct(st.disk), st.disk.is_none());
    small_gauge(p, &m, GAUGES[7], r[7], Some(90.0), shown[7], "C:", &pct(st.disk_c), st.disk_c.is_none());
    clock(p, &m, GAUGES[8]);

    // ---- spie ----
    const AMB: Color32 = Color32::from_rgb(255, 170, 30);
    const RED: Color32 = Color32::from_rgb(255, 48, 36);
    const GRN: Color32 = Color32::from_rgb(70, 230, 90);
    const BLU: Color32 = Color32::from_rgb(60, 130, 255);
    let low_batt = st.battery.is_some_and(|(b, ch)| b < 20.0 && !ch);
    let lamps: [(bool, Color32, &str, bool); 10] = [
        (ready && st.cpu > 90.0, AMB, theme::ic::CPU, false),
        (ready && st.ram > 90.0, AMB, "\u{E964}", false),
        (st.gpu.is_some_and(|g| g > 95.0), RED, "\u{E9CA}", false),
        (st.disk.is_some_and(|d| d > 90.0), AMB, "\u{EDA2}", false),
        (st.disk_c.is_some_and(|d| d > 90.0), RED, "C:", true),
        (st.net_mbit.is_some_and(|n| n > 5.0), GRN, theme::ic::WIFI, false),
        (st.docker.is_some_and(|n| n > 0), BLU, theme::ic::BOX, false),
        (low_batt && blink, RED, "\u{E83F}", false),
        (f.office.call, RED, theme::ic::PHONE, false),
        (f.office.coffee && blink, AMB, theme::ic::COFFEE, false),
    ];
    for (i, (on, col, g, text)) in lamps.iter().enumerate() {
        lamp(p, &m, light_rect(i), *on, *col, g, *text);
    }

    // ---- console di destra: display e interruttori ----
    lcd(p, &m, st);
    let charging = st.battery.is_some_and(|(_, c)| c);
    let rockers: [(Option<Color32>, &str, bool); 12] = [
        (st.wsl_gb.map(|_| GRN), "WSL", true),
        (st.docker.map(|_| GRN), theme::ic::BOX, false),
        (st.net_mbit.filter(|n| *n > 1.0).map(|_| AMB), theme::ic::WIFI, false),
        (st.battery.map(|_| if charging { GRN } else { AMB }), "\u{E83F}", false),
        (f.office.call.then_some(RED), theme::ic::PHONE, false),
        (f.office.coffee.then_some(AMB), theme::ic::COFFEE, false),
        (f.office.auto_detect.then_some(GRN), "AUT", true),
        (None, "\u{EA80}", false),
        (None, theme::ic::SYNC, false),
        (Some(GRN), theme::ic::POWER, false),
        (None, theme::ic::BELL, false),
        (None, "\u{E72E}", false),
    ];
    for (i, (led, g, text)) in rockers.iter().enumerate() {
        rocker(p, &m, rocker_rect(i), *led, g, *text);
    }
    // console di sinistra: tre interruttori di servizio
    for (i, g) in ["\u{EA80}", "\u{E706}", "\u{E708}"].iter().enumerate() {
        let r = Rect::from_center_size(Pos2::new(150.0 + 60.0 * i as f32, 872.0), Vec2::new(46.0, 64.0));
        rocker(p, &m, r, None, g, false);
    }
    glass(p, &m);
}

// ---------------------------------------------------------------- immagini statiche

/// Immagine di uno sfondo per la galleria delle Impostazioni (con i valori di esempio
/// per il cruscotto).
pub fn thumbnail(gpu: &mut Gpu, wp: &Wallpaper, params: Params) -> Option<Vec<u8>> {
    if wp.id != ID {
        return gpu.render_offscreen(wp, params, None);
    }
    render_static(gpu, wp, params, demo_stats())
}

/// Cruscotto fermo, con le lancette sui valori dati (anteprime, --snapshot).
pub fn render_static(gpu: &mut Gpu, wp: &Wallpaper, mut params: Params, stats: DashStats) -> Option<Vec<u8>> {
    params.dash = [params.time * 2.0, 0.0, 0.0, 0.0];
    let mut ov = DashOverlay::new(gpu, gpu.format);
    let (t, r) = (targets(&stats), ranges(&stats));
    let mut shown = [0.0; 9];
    for i in 0..8 {
        shown[i] = t[i].min(r[i]);
    }
    let size_px = [(params.res[0] * params.scale).round() as u32, (params.res[1] * params.scale).round() as u32];
    let frame = DashFrame {
        size_px,
        scale: params.scale,
        offset: params.offset,
        canvas: params.canvas,
        stats,
        office: DashOffice { call: true, coffee: false, auto_detect: true },
        since: 60.0,
    };
    let mut extra = |g: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView| ov.draw(g, enc, view, &frame, &shown);
    gpu.render_offscreen(wp, params, Some(&mut extra))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conta_i_container() {
        let ok = b"HTTP/1.0 200 OK\r\nContent-Type: application/json\r\n\r\n[{\"Id\":\"a\"},{\"Id\":\"b\"}]\n";
        assert_eq!(count_containers(ok), Some(2));
        assert_eq!(count_containers(b"HTTP/1.0 200 OK\r\n\r\n[]"), Some(0));
        assert_eq!(count_containers(b"HTTP/1.0 500 Errore\r\n\r\n{}"), None);
        assert_eq!(count_containers(b"spazzatura"), None);
    }

    #[test]
    fn gpu_per_motore() {
        let v = vec![
            ("pid_1_luid_0x0_0xA_phys_0_eng_0_engtype_3D".to_string(), 20.0),
            ("pid_2_luid_0x0_0xA_phys_0_eng_0_engtype_3D".to_string(), 15.0),
            ("pid_2_luid_0x0_0xA_phys_0_eng_1_engtype_3D".to_string(), 30.0),
        ];
        assert_eq!(gpu_from_engines(&v), 35.0);
        assert_eq!(gpu_from_engines(&[]), 0.0);
    }

    #[test]
    fn giro_di_prova() {
        assert_eq!(sweep_k(0.0), 0.0);
        assert!(sweep_k(1.4) > 0.95);
        assert_eq!(sweep_k(5.0), -1.0);
    }

    #[test]
    fn costanti_shader() {
        let s = wgsl_consts();
        assert!(s.contains("const GAUGES = array<vec4f, 9>("));
        assert_eq!(s.matches("vec4f(").count(), 9 + 12 + 1);
    }

    #[test]
    fn mappa_ancorata_in_basso() {
        let f = DashFrame { size_px: [2560, 1600], scale: 1.0, offset: [0.0, 0.0], canvas: [2560.0, 1600.0], stats: DashStats::default(), office: DashOffice::default(), since: 0.0 };
        let m = Map::new(&f);
        let bottom = m.p(960.0, 1080.0);
        assert!((bottom.y - 1600.0).abs() < 0.01);
        assert!((bottom.x - 1280.0).abs() < 0.01);
    }
}
