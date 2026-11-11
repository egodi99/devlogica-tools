//! Linguaggio visivo delle finestre: colori, caratteri e i pochi controlli su misura
//! (interruttori, selettori a segmenti, schede, righe delle impostazioni).
//!
//! Su Windows i caratteri sono quelli del sistema (Segoe UI e Segoe Fluent Icons), letti
//! dalla cartella Fonts all'avvio: niente file da includere nell'eseguibile. Dove mancano
//! (macOS, Windows senza il carattere delle icone) si usano quelli di egui e le icone spariscono.

use egui::{Color32, CornerRadius, FontFamily, FontId, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, Vec2};
use std::sync::{Arc, OnceLock};

// ---------- colori ----------
/// Fondo delle finestre.
pub const BG: Color32 = Color32::from_rgb(17, 17, 20);
/// Barra laterale e intestazioni.
pub const BG_SIDE: Color32 = Color32::from_rgb(22, 22, 26);
/// Schede e riquadri.
pub const SURFACE: Color32 = Color32::from_rgb(28, 28, 33);
pub const SURFACE_HI: Color32 = Color32::from_rgb(36, 36, 42);
pub const HOVER: Color32 = Color32::from_rgb(44, 44, 51);
pub const BORDER: Color32 = Color32::from_rgb(46, 46, 54);
pub const BORDER_SOFT: Color32 = Color32::from_rgb(36, 36, 42);

pub const TEXT: Color32 = Color32::from_rgb(237, 237, 242);
pub const TEXT_2: Color32 = Color32::from_rgb(160, 160, 172);
pub const TEXT_3: Color32 = Color32::from_rgb(112, 112, 124);

/// Viola DevLogica, schiarito per stare bene sul fondo scuro.
pub const ACCENT: Color32 = Color32::from_rgb(150, 84, 226);
pub const ACCENT_HI: Color32 = Color32::from_rgb(168, 108, 238);
pub const ACCENT_TEXT: Color32 = Color32::from_rgb(198, 160, 245);
pub const ACCENT_BG: Color32 = Color32::from_rgb(40, 30, 58);

pub const RED: Color32 = Color32::from_rgb(242, 76, 68);
pub const RED_TEXT: Color32 = Color32::from_rgb(255, 140, 130);
pub const RED_BG: Color32 = Color32::from_rgb(52, 24, 26);
pub const AMBER: Color32 = Color32::from_rgb(232, 166, 62);
pub const AMBER_BG: Color32 = Color32::from_rgb(50, 38, 22);
pub const GREEN: Color32 = Color32::from_rgb(64, 190, 104);
pub const GREEN_BG: Color32 = Color32::from_rgb(24, 46, 32);

pub const R_SM: u8 = 6;
pub const R_MD: u8 = 8;
pub const R_LG: u8 = 12;

// ---------- icone (Segoe Fluent Icons / Segoe MDL2 Assets) ----------
pub mod ic {
    pub const PHONE: &str = "\u{E717}";
    pub const WARNING: &str = "\u{E7BA}";
    pub const CPU: &str = "\u{E950}";
    pub const DIAGNOSTIC: &str = "\u{E9D9}";
    pub const WIFI: &str = "\u{E701}";
    pub const POWER: &str = "\u{E7E8}";
    pub const GEAR: &str = "\u{E713}";
    pub const SYNC: &str = "\u{E895}";
    pub const CHECK: &str = "\u{E73E}";
    pub const CHEVRON_R: &str = "\u{E76C}";
    pub const BACK: &str = "\u{E72B}";
    pub const PERSON: &str = "\u{E77B}";
    pub const PEOPLE: &str = "\u{E716}";
    pub const CHART: &str = "\u{E9D2}";
    pub const CLOCK: &str = "\u{E823}";
    pub const COFFEE: &str = "\u{EC32}";
    pub const FOLDER: &str = "\u{E8B7}";
    pub const HOME: &str = "\u{E80F}";
    pub const GAUGE: &str = "\u{EC4A}";
    pub const PICTURE: &str = "\u{E91B}";
    pub const MIC: &str = "\u{E720}";
    pub const VOLUME: &str = "\u{E767}";
    pub const OPEN: &str = "\u{E8A7}";
    pub const BELL: &str = "\u{EA8F}";
    pub const PAUSE: &str = "\u{E769}";
    pub const PLAY: &str = "\u{E768}";
    pub const DOWNLOAD: &str = "\u{E896}";
    pub const BOX: &str = "\u{E7B8}";
    pub const SLIDERS: &str = "\u{E9E9}";
}

// Caratteri inclusi nell'eseguibile (~135 KB), usati dove mancano quelli di sistema (macOS,
// Windows 10 senza Segoe Fluent Icons). Licenze in assets/fonts.
// - icone.ttf: icone Lucide (ISC) già rimappate sugli stessi codici di Segoe Fluent Icons,
//   così ogni `ic::*` funziona con entrambi e si ripiega glifo per glifo;
// - Inter (OFL) per i testi; Barlow Semi Condensed (OFL) per i numeri del cruscotto.
const ICONS_TTF: &[u8] = include_bytes!("../assets/fonts/icone.ttf");
const INTER_REGULAR: &[u8] = include_bytes!("../assets/fonts/inter-regular.ttf");
const INTER_SEMIBOLD: &[u8] = include_bytes!("../assets/fonts/inter-semibold.ttf");
pub const BARLOW: &[u8] = include_bytes!("../assets/fonts/barlow-cruscotto.ttf");

struct SystemFonts {
    regular: Option<Arc<egui::FontData>>,
    semibold: Option<Arc<egui::FontData>>,
    icons: Option<Arc<egui::FontData>>,
}

/// Vero se si devono ignorare i caratteri di sistema (DLT_FONTS=inclusi: anteprima dell'aspetto su macOS).
fn only_embedded() -> bool {
    std::env::var("DLT_FONTS").is_ok_and(|v| v == "inclusi")
}

fn font_dir() -> Option<std::path::PathBuf> {
    if only_embedded() {
        return None;
    }
    if cfg!(windows) {
        let win = std::env::var_os("WINDIR").map(std::path::PathBuf::from).unwrap_or_else(|| "C:\\Windows".into());
        Some(win.join("Fonts"))
    } else {
        // Solo per le prove su Linux/WSL: i caratteri di Windows, se raggiungibili.
        std::env::var_os("DLT_FONT_DIR").map(std::path::PathBuf::from)
    }
}

/// Letti una volta sola e condivisi da tutte le finestre.
fn system_fonts() -> &'static SystemFonts {
    static F: OnceLock<SystemFonts> = OnceLock::new();
    F.get_or_init(|| {
        let load = |names: &[&str]| -> Option<Arc<egui::FontData>> {
            let dir = font_dir()?;
            names.iter().find_map(|n| std::fs::read(dir.join(n)).ok()).map(|b| Arc::new(egui::FontData::from_owned(b)))
        };
        SystemFonts {
            regular: load(&["segoeui.ttf"]),
            semibold: load(&["seguisb.ttf", "segoeuib.ttf"]),
            // solo Segoe Fluent Icons (Windows 11): a Segoe MDL2 mancano alcune icone, meglio le nostre
            icons: load(&["SegoeIcons.ttf"]),
        }
    })
}

/// Un carattere di sistema qualsiasi (il primo trovato tra i nomi dati), letto una volta sola.
pub fn system_font(names: &[&str]) -> Option<Arc<egui::FontData>> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: OnceLock<Mutex<HashMap<String, Option<Arc<egui::FontData>>>>> = OnceLock::new();
    let key = names.join("|");
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut c = cache.lock().ok()?;
    c.entry(key)
        .or_insert_with(|| {
            let dir = font_dir()?;
            names.iter().find_map(|n| std::fs::read(dir.join(n)).ok()).map(|b| Arc::new(egui::FontData::from_owned(b)))
        })
        .clone()
}

pub fn semibold_family() -> FontFamily {
    FontFamily::Name("semibold".into())
}

fn icon_family() -> FontFamily {
    FontFamily::Name("icons".into())
}

/// Aggiunge le icone (di sistema se ci sono, poi le nostre come ripiego) a `defs`;
/// restituisce i nomi da mettere in testa alla famiglia delle icone.
pub fn add_icon_fonts(defs: &mut egui::FontDefinitions) -> Vec<String> {
    let mut names = Vec::new();
    if let Some(i) = &system_fonts().icons {
        defs.font_data.insert("fluent".into(), i.clone());
        names.push("fluent".into());
    }
    defs.font_data.insert("icone".into(), Arc::new(egui::FontData::from_static(ICONS_TTF)));
    names.push("icone".into());
    names
}

pub fn install_fonts(ctx: &egui::Context) {
    let sys = system_fonts();
    let mut defs = egui::FontDefinitions::default();
    let base: Vec<String> = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let regular = sys.regular.clone().unwrap_or_else(|| Arc::new(egui::FontData::from_static(INTER_REGULAR)));
    let semibold = sys.semibold.clone().unwrap_or_else(|| Arc::new(egui::FontData::from_static(INTER_SEMIBOLD)));
    defs.font_data.insert("testo".into(), regular);
    defs.font_data.insert("testo-sb".into(), semibold);
    let icon_names = add_icon_fonts(&mut defs);
    let mut prop: Vec<String> = std::iter::once("testo".to_string()).chain(base.iter().cloned()).collect();
    let mut sb: Vec<String> = ["testo-sb", "testo"].iter().map(|s| s.to_string()).chain(base.iter().cloned()).collect();
    // le icone anche come ripiego dei testi normali, per usarle dentro le etichette
    prop.extend(icon_names.iter().cloned());
    sb.extend(icon_names.iter().cloned());
    let icons: Vec<String> = icon_names.into_iter().chain(base).collect();
    defs.families.insert(FontFamily::Proportional, prop);
    defs.families.insert(semibold_family(), sb);
    defs.families.insert(icon_family(), icons);
    ctx.set_fonts(defs);
}

/// Le icone ci sono sempre: quelle incluse fanno da ripiego.
pub fn has_icons() -> bool {
    true
}

pub fn install(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut v = egui::Visuals::dark();
    v.panel_fill = BG;
    v.window_fill = SURFACE;
    v.extreme_bg_color = Color32::from_rgb(14, 14, 17);
    v.text_edit_bg_color = Some(Color32::from_rgb(14, 14, 17));
    v.faint_bg_color = SURFACE;
    v.override_text_color = None;
    v.selection.bg_fill = ACCENT;
    v.selection.stroke = Stroke::new(1.0, ACCENT_TEXT);
    v.hyperlink_color = ACCENT_TEXT;
    v.window_corner_radius = CornerRadius::same(R_LG);
    v.menu_corner_radius = CornerRadius::same(R_MD);
    v.window_stroke = Stroke::new(1.0, BORDER);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;
    v.indent_has_left_vline = false;
    let w = &mut v.widgets;
    for (s, fill, stroke, fg) in [
        (&mut w.noninteractive, SURFACE, BORDER_SOFT, TEXT_2),
        (&mut w.inactive, SURFACE_HI, BORDER, TEXT),
        (&mut w.hovered, HOVER, Color32::from_rgb(64, 64, 74), TEXT),
        (&mut w.active, Color32::from_rgb(54, 54, 62), ACCENT, TEXT),
        (&mut w.open, SURFACE_HI, BORDER, TEXT),
    ] {
        s.bg_fill = fill;
        s.weak_bg_fill = fill;
        s.bg_stroke = Stroke::new(1.0, stroke);
        s.fg_stroke = Stroke::new(1.0, fg);
        s.corner_radius = CornerRadius::same(R_SM);
        s.expansion = 0.0;
    }
    ctx.set_visuals(v);
    ctx.all_styles_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 8.0);
        s.spacing.button_padding = Vec2::new(14.0, 7.0);
        s.spacing.interact_size.y = 22.0;
        s.spacing.slider_rail_height = 4.0;
        use egui::TextStyle::*;
        s.text_styles.insert(Body, FontId::proportional(14.0));
        s.text_styles.insert(Button, FontId::proportional(14.0));
        s.text_styles.insert(Small, FontId::proportional(12.0));
        s.text_styles.insert(Heading, FontId::new(22.0, semibold_family()));
        s.text_styles.insert(Monospace, FontId::monospace(13.0));
    });
}

// ---------- testo ----------

pub fn strong(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(semibold_family()).size(size).color(TEXT)
}

/// Etichetta piccola in maiuscolo sopra un gruppo.
pub fn eyebrow(text: &str) -> RichText {
    RichText::new(text.to_uppercase()).family(semibold_family()).size(11.0).color(TEXT_3).extra_letter_spacing(0.6)
}

pub fn icon(code: &str, size: f32, color: Color32) -> RichText {
    let t = if has_icons() { code } else { "" };
    RichText::new(t).family(icon_family()).size(size).color(color)
}

/// Disegna un'icona centrata in un punto (niente se il carattere delle icone manca).
pub fn paint_icon(p: &egui::Painter, center: Pos2, code: &str, size: f32, color: Color32) {
    if has_icons() {
        p.text(center, egui::Align2::CENTER_CENTER, code, FontId::new(size, icon_family()), color);
    }
}

// ---------- controlli ----------

/// Interruttore acceso/spento in stile Windows 11.
pub fn toggle(ui: &mut egui::Ui, on: &mut bool) -> Response {
    let size = Vec2::new(40.0, 20.0);
    let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let t = ui.ctx().animate_bool_responsive(resp.id, *on);
    let p = ui.painter();
    let r = rect.height() / 2.0;
    let (fill, stroke) = if *on {
        (if resp.hovered() { ACCENT_HI } else { ACCENT }, Stroke::NONE)
    } else {
        (if resp.hovered() { HOVER } else { Color32::TRANSPARENT }, Stroke::new(1.0, TEXT_2))
    };
    p.rect(rect, r, fill, stroke, StrokeKind::Inside);
    let knob_r = if resp.is_pointer_button_down_on() { 7.0 } else if resp.hovered() { 6.5 } else { 6.0 };
    let x = egui::lerp((rect.left() + r)..=(rect.right() - r), t);
    p.circle_filled(Pos2::new(x, rect.center().y), knob_r, if *on { Color32::WHITE } else { TEXT_2 });
    resp
}

/// Selettore a segmenti: restituisce true se la scelta è cambiata.
pub fn segmented<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, options: &[(T, &str)]) -> bool {
    let mut changed = false;
    let font = FontId::proportional(13.0);
    let pad = 12.0;
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, l)| ui.painter().layout_no_wrap(l.to_string(), font.clone(), TEXT).size().x + pad * 2.0)
        .collect();
    let total = Vec2::new(widths.iter().sum::<f32>() + 4.0, 30.0);
    let (rect, _) = ui.allocate_exact_size(total, Sense::hover());
    ui.painter().rect(rect, R_SM, Color32::from_rgb(14, 14, 17), Stroke::new(1.0, BORDER_SOFT), StrokeKind::Inside);
    let mut x = rect.left() + 2.0;
    for ((v, label), w) in options.iter().zip(&widths) {
        let r = Rect::from_min_size(Pos2::new(x, rect.top() + 2.0), Vec2::new(*w, rect.height() - 4.0));
        let resp = ui.interact(r, ui.id().with(("seg", label)), Sense::click());
        let sel = *value == *v;
        if sel {
            ui.painter().rect(r, R_SM - 2, SURFACE_HI, Stroke::new(1.0, BORDER), StrokeKind::Inside);
        } else if resp.hovered() {
            ui.painter().rect_filled(r, R_SM - 2, Color32::from_rgb(26, 26, 31));
        }
        let color = if sel { TEXT } else { TEXT_2 };
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, *label, font.clone(), color);
        if resp.clicked() && !sel {
            *value = *v;
            changed = true;
        }
        x += w;
    }
    changed
}

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Primary,
    Secondary,
    Danger,
    Ghost,
    Amber,
}

pub fn button(ui: &mut egui::Ui, kind: Kind, text: &str) -> Response {
    button_icon(ui, kind, None, text)
}

pub fn button_icon(ui: &mut egui::Ui, kind: Kind, icon_code: Option<&str>, text: &str) -> Response {
    let (fill, fg, stroke) = match kind {
        Kind::Primary => (ACCENT, Color32::WHITE, Stroke::NONE),
        Kind::Secondary => (SURFACE_HI, TEXT, Stroke::new(1.0, BORDER)),
        Kind::Danger => (RED_BG, RED_TEXT, Stroke::new(1.0, Color32::from_rgb(92, 40, 42))),
        Kind::Ghost => (Color32::TRANSPARENT, TEXT_2, Stroke::NONE),
        Kind::Amber => (AMBER, Color32::from_rgb(30, 20, 6), Stroke::NONE),
    };
    let mut job = egui::text::LayoutJob::default();
    if let Some(c) = icon_code.filter(|_| has_icons()) {
        job.append(c, 0.0, egui::TextFormat { font_id: FontId::new(13.0, icon_family()), color: fg, valign: egui::Align::Center, ..Default::default() });
        job.append(" ", 0.0, egui::TextFormat { font_id: FontId::proportional(14.0), color: fg, ..Default::default() });
    }
    job.append(text, 2.0, egui::TextFormat { font_id: FontId::new(14.0, semibold_family()), color: fg, valign: egui::Align::Center, ..Default::default() });
    let resp = ui.add(egui::Button::new(job).fill(fill).stroke(stroke).corner_radius(R_SM).min_size(Vec2::new(0.0, 32.0)));
    if resp.hovered() {
        let r = resp.rect;
        let over = if kind == Kind::Ghost { Color32::from_white_alpha(14) } else { Color32::from_white_alpha(10) };
        ui.painter().rect_filled(r, R_SM, over);
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Riquadro con fondo e bordo leggero.
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER_SOFT))
        .corner_radius(R_MD)
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// Gruppo di righe impostazioni dentro un riquadro, separate da linee sottili.
pub fn group(ui: &mut egui::Ui, add: impl FnOnce(&mut Rows)) {
    egui::Frame::new()
        .fill(SURFACE)
        .stroke(Stroke::new(1.0, BORDER_SOFT))
        .corner_radius(R_MD)
        .inner_margin(egui::Margin::symmetric(16, 4))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut rows = Rows { ui, first: true };
            add(&mut rows);
        });
}

pub struct Rows<'a> {
    pub ui: &'a mut egui::Ui,
    first: bool,
}

impl Rows<'_> {
    fn sep(&mut self) {
        if !self.first {
            let r = self.ui.available_rect_before_wrap();
            self.ui.painter().hline(r.left()..=r.right(), r.top(), Stroke::new(1.0, BORDER_SOFT));
        }
        self.first = false;
    }

    /// Riga: icona, titolo e descrizione a sinistra, controllo a destra.
    pub fn row(&mut self, icon_code: Option<&str>, title: &str, desc: &str, control: impl FnOnce(&mut egui::Ui)) {
        self.sep();
        let ui = &mut *self.ui;
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.set_min_height(34.0);
            if let Some(c) = icon_code.filter(|_| has_icons()) {
                let (r, _) = ui.allocate_exact_size(Vec2::new(28.0, 34.0), Sense::hover());
                paint_icon(ui.painter(), Pos2::new(r.left() + 10.0, r.center().y), c, 16.0, TEXT_2);
            }
            let right_w = 260.0f32.min(ui.available_width() * 0.5);
            ui.allocate_ui_with_layout(Vec2::new(ui.available_width() - right_w, 34.0), egui::Layout::top_down(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(RichText::new(title).size(14.0).color(TEXT));
                if !desc.is_empty() {
                    ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(TEXT_2)).wrap());
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                control(ui);
            });
        });
        self.ui.add_space(12.0);
    }

    /// Contenuto libero a tutta larghezza.
    pub fn block(&mut self, add: impl FnOnce(&mut egui::Ui)) {
        self.sep();
        self.ui.add_space(12.0);
        add(self.ui);
        self.ui.add_space(12.0);
    }
}

/// Titolo di una sezione sopra un gruppo.
pub fn section_title(ui: &mut egui::Ui, text: &str) {
    ui.add_space(14.0);
    ui.label(strong(text, 14.0));
    ui.add_space(2.0);
}

/// Colore stabile per un nome (iniziale nei cerchi).
pub fn person_color(name: &str) -> Color32 {
    let h = name.bytes().fold(2166136261u32, |h, b| (h ^ b as u32).wrapping_mul(16777619));
    const P: [Color32; 8] = [
        Color32::from_rgb(124, 92, 214),
        Color32::from_rgb(46, 134, 193),
        Color32::from_rgb(38, 150, 120),
        Color32::from_rgb(196, 98, 54),
        Color32::from_rgb(178, 72, 132),
        Color32::from_rgb(84, 112, 204),
        Color32::from_rgb(160, 120, 40),
        Color32::from_rgb(92, 140, 70),
    ];
    P[(h % P.len() as u32) as usize]
}

pub fn avatar(ui: &mut egui::Ui, name: &str, size: f32) -> Rect {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    paint_avatar(ui.painter(), rect.center(), name, size);
    rect
}

pub fn paint_avatar(p: &egui::Painter, c: Pos2, name: &str, size: f32) {
    let col = person_color(name);
    p.circle_filled(c, size / 2.0, col);
    let initial = name.trim().chars().next().map(|c| c.to_uppercase().to_string()).unwrap_or_else(|| "?".into());
    p.text(c, egui::Align2::CENTER_CENTER, initial, FontId::new(size * 0.42, semibold_family()), Color32::WHITE);
}

/// Etichetta di stato colorata ("In call", "Viene"…).
pub fn chip(ui: &mut egui::Ui, text: &str, fg: Color32, bg: Color32) -> Response {
    let font = FontId::new(12.0, semibold_family());
    let g = ui.painter().layout_no_wrap(text.to_string(), font, fg);
    let size = g.size() + Vec2::new(16.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::same((size.y / 2.0) as u8), bg);
    ui.painter().galley(rect.center() - g.size() / 2.0, g, fg);
    resp
}

/// Pallino di stato, con alone che pulsa se `pulse`.
pub fn status_dot(ui: &mut egui::Ui, color: Color32, pulse: bool) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    let c = rect.center();
    if pulse {
        let t = ui.input(|i| i.time);
        let k = ((t % 1.6) / 1.6) as f32;
        ui.painter().circle_filled(c, 4.0 + 6.0 * k, color.gamma_multiply(0.45 * (1.0 - k)));
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(33));
    }
    ui.painter().circle_filled(c, 4.0, color);
}

/// Logo DevLogica (il simbolo) come texture, caricata una volta per finestra.
pub fn mark_texture(ctx: &egui::Context) -> egui::TextureHandle {
    let id = egui::Id::new("devlogica-mark");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let img = image::load_from_memory(include_bytes!("../assets/app-icon-1024.png")).expect("icona").to_rgba8();
    let img = image::imageops::resize(&img, 128, 128, image::imageops::FilterType::Lanczos3);
    let color = egui::ColorImage::from_rgba_unmultiplied([128, 128], img.as_raw());
    let tex = ctx.load_texture("devlogica-mark", color, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, tex.clone()));
    tex
}

pub fn mark(ui: &mut egui::Ui, size: f32) {
    let tex = mark_texture(ui.ctx());
    ui.add(egui::Image::new(&tex).fit_to_exact_size(Vec2::splat(size)).corner_radius(CornerRadius::same((size * 0.22) as u8)));
}

/// Icona della finestra (barra del titolo e barra delle applicazioni).
pub fn window_icon() -> Option<winit::window::Icon> {
    let img = image::load_from_memory(include_bytes!("../assets/app-icon-1024.png")).ok()?.to_rgba8();
    let img = image::imageops::resize(&img, 64, 64, image::imageops::FilterType::Lanczos3);
    winit::window::Icon::from_rgba(img.into_raw(), 64, 64).ok()
}
