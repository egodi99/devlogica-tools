//! Indicatori sullo sfondo (CPU, RAM, orologio, logo): logica pura, senza dipendenze
//! dal sistema. Ordine e blocchi per lato, spazio disponibile accanto al Dock, formati.

use crate::config::{IndicatorStyle, Indicators, Side};

/// Margine tra il bordo dello schermo e il primo elemento, in punti.
pub const EDGE_MARGIN: f32 = 16.0;
/// Distanza minima tra l'ultimo elemento e il Dock, in punti.
pub const DOCK_MARGIN: f32 = 12.0;
/// Senza la posizione esatta del Dock si lascia libera questa frazione centrale dello schermo.
pub const FALLBACK_BAND: f32 = 0.70;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Logo,
    Clock,
    Ram,
    Cpu,
}

/// Un blocco disegnato come unità: un elemento singolo, oppure (stile C) l'orologio grande
/// con sotto le righe piccole degli elementi dello stesso lato.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    Single(Item),
    ClockStack(Vec<Item>),
}

/// Blocchi di un lato, in ordine dal bordo dello schermo verso il Dock.
pub fn blocks(ind: &Indicators, side: Side) -> Vec<Block> {
    if !ind.enabled || side == Side::Hidden {
        return Vec::new();
    }
    let items: Vec<Item> = [(Item::Logo, ind.logo), (Item::Clock, ind.clock), (Item::Ram, ind.ram), (Item::Cpu, ind.cpu)]
        .into_iter()
        .filter(|(_, s)| *s == side)
        .map(|(i, _)| i)
        .collect();
    if ind.style == IndicatorStyle::BigClock && items.contains(&Item::Clock) {
        let mut out = Vec::new();
        if items.contains(&Item::Logo) {
            out.push(Block::Single(Item::Logo));
        }
        out.push(Block::ClockStack(items.iter().copied().filter(|i| matches!(i, Item::Ram | Item::Cpu)).collect()));
        return out;
    }
    items.into_iter().map(Block::Single).collect()
}

/// Posizione del Dock in coordinate locali dello schermo, in punti.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DockPlace {
    /// Dock in basso, da `x0` a `x1`, con il bordo superiore a `top`.
    Bottom { x0: f32, x1: f32, top: f32 },
    /// Dock sul lato sinistro o destro dello schermo: niente fascia centrale da evitare.
    Side,
    /// Posizione sconosciuta (nessun permesso, Dock nascosto): si lascia libera una fascia fissa.
    Unknown,
}

/// Rettangolo in punti globali, con l'origine in alto a sinistra dello schermo principale
/// (le stesse coordinate dell'Accessibilità di macOS e di winit in punti logici).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GRect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Schermo che ha il Dock e, se noto, il rettangolo del Dock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockInfo {
    pub screen: GRect,
    pub dock: Option<GRect>,
    pub at_bottom: bool,
}

/// Converte il rettangolo del Dock in coordinate locali dello schermo.
pub fn dock_place(screen: GRect, dock: Option<GRect>, at_bottom: bool) -> DockPlace {
    match dock {
        None => DockPlace::Unknown,
        Some(_) if !at_bottom => DockPlace::Side,
        Some(d) => DockPlace::Bottom { x0: (d.x - screen.x) as f32, x1: (d.x + d.w - screen.x) as f32, top: (d.y - screen.y) as f32 },
    }
}

/// Larghezza utile a sinistra e a destra del Dock, già al netto dei margini.
pub fn free_widths(screen_w: f32, dock: DockPlace) -> (f32, f32) {
    let (l, r) = match dock {
        DockPlace::Bottom { x0, x1, .. } => (x0 - EDGE_MARGIN - DOCK_MARGIN, screen_w - x1 - EDGE_MARGIN - DOCK_MARGIN),
        DockPlace::Side => (screen_w / 2.0 - EDGE_MARGIN, screen_w / 2.0 - EDGE_MARGIN),
        DockPlace::Unknown => {
            let w = screen_w * (1.0 - FALLBACK_BAND) / 2.0 - EDGE_MARGIN - DOCK_MARGIN;
            (w, w)
        }
    };
    (l.max(0.0), r.max(0.0))
}

/// Centro verticale degli indicatori: la fascia del Dock, altrimenti appena sopra il bordo.
/// Un blocco alto (orologio grande) non deve mai uscire dal fondo dello schermo.
pub fn center_y(dock: DockPlace, screen_h: f32, max_h: f32) -> f32 {
    match dock {
        DockPlace::Bottom { top, .. } if top < screen_h => ((top + screen_h) / 2.0).min(screen_h - 4.0 - max_h / 2.0),
        _ => screen_h - EDGE_MARGIN - max_h / 2.0,
    }
}

/// Scala di disegno dello sfondo quando ci sono gli indicatori: il testo non va mai disegnato
/// con meno di un pixel per punto (qualità "Ridotta" su uno schermo non Retina).
pub fn indicator_scale(scale: f32, scale_factor: f32, indicators_on: bool) -> f32 {
    if indicators_on && scale * scale_factor < 1.0 {
        (1.0 / scale_factor.max(0.01)).min(1.0)
    } else {
        scale
    }
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn contains(r: &GRect, x: f64, y: f64) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

/// Sceglie lo schermo del Dock. `screens` = (frame, visibleFrame) in punti globali,
/// `ax` = barra del Dock letta con l'Accessibilità, `autohide` = Dock nascosto automaticamente.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn choose_dock(screens: &[(GRect, GRect)], ax: Option<GRect>, autohide: bool) -> Option<DockInfo> {
    let first = screens.first()?;
    if let Some(d) = ax.filter(|_| !autohide) {
        let (cx, cy) = (d.x + d.w / 2.0, d.y + d.h / 2.0);
        if let Some((frame, _)) = screens.iter().find(|(f, _)| contains(f, cx, cy)) {
            return Some(DockInfo { screen: *frame, dock: Some(d), at_bottom: d.w > d.h });
        }
    }
    // Senza misura esatta: lo schermo la cui area utile non arriva fino in fondo (o ai lati).
    for (f, v) in screens {
        if (f.y + f.h) - (v.y + v.h) > 1.0 {
            return Some(DockInfo { screen: *f, dock: None, at_bottom: true });
        }
        if v.w + 1.0 < f.w {
            return Some(DockInfo { screen: *f, dock: None, at_bottom: false });
        }
    }
    Some(DockInfo { screen: first.0, dock: None, at_bottom: true })
}

/// Quanti blocchi, partendo dal bordo dello schermo, stanno in `available` punti.
/// Ci si ferma al primo che non ci sta: quelli più vicini al Dock vengono omessi interi.
pub fn fit(widths: &[f32], available: f32, gap: f32) -> usize {
    let mut used = 0.0;
    for (i, w) in widths.iter().enumerate() {
        let next = used + if i > 0 { gap } else { 0.0 } + w;
        if next > available {
            return i;
        }
        used = next;
    }
    widths.len()
}

pub fn fmt_cpu(pct: f32) -> String {
    let p = if pct.is_finite() { pct.clamp(0.0, 100.0) } else { 0.0 };
    format!("CPU {}%", p.round() as u32)
}

pub fn fmt_ram(used: u64, total: u64) -> String {
    const GIB: f64 = (1u64 << 30) as f64;
    let used = format!("{:.1}", used.min(total) as f64 / GIB).replace('.', ",");
    format!("RAM {used} / {} GB", (total as f64 / GIB).round() as u64)
}

pub fn fmt_clock(h: u32, m: u32) -> String {
    format!("{h:02}:{m:02}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use Item::*;

    fn s(items: &[Item]) -> Vec<Block> {
        items.iter().map(|i| Block::Single(*i)).collect()
    }

    #[test]
    fn blocks_ordine() {
        let ind = Indicators::default();
        assert_eq!(blocks(&ind, Side::Left), s(&[Logo, Clock]));
        assert_eq!(blocks(&ind, Side::Right), s(&[Ram, Cpu]));
    }

    #[test]
    fn blocks_tutti_a_sinistra() {
        let ind = Indicators { logo: Side::Left, clock: Side::Left, ram: Side::Left, cpu: Side::Left, ..Default::default() };
        assert_eq!(blocks(&ind, Side::Left), s(&[Logo, Clock, Ram, Cpu]));
        assert_eq!(blocks(&ind, Side::Right), vec![]);
    }

    #[test]
    fn blocks_nascosti() {
        let off = Indicators { enabled: false, ..Default::default() };
        assert_eq!(blocks(&off, Side::Left), vec![]);
        assert_eq!(blocks(&off, Side::Right), vec![]);
        let no_cpu = Indicators { cpu: Side::Hidden, ..Default::default() };
        assert_eq!(blocks(&no_cpu, Side::Left), s(&[Logo, Clock]));
        assert_eq!(blocks(&no_cpu, Side::Right), s(&[Ram]));
    }

    #[test]
    fn blocks_stile_c() {
        let all_left = Indicators { style: IndicatorStyle::BigClock, logo: Side::Left, clock: Side::Left, ram: Side::Left, cpu: Side::Left, ..Default::default() };
        assert_eq!(blocks(&all_left, Side::Left), vec![Block::Single(Logo), Block::ClockStack(vec![Ram, Cpu])]);
        let split = Indicators { style: IndicatorStyle::BigClock, clock: Side::Left, ram: Side::Left, cpu: Side::Right, ..Default::default() };
        assert_eq!(blocks(&split, Side::Right), s(&[Cpu]));
        assert_eq!(blocks(&split, Side::Left), vec![Block::Single(Logo), Block::ClockStack(vec![Ram])]);
        let no_clock = Indicators { style: IndicatorStyle::BigClock, clock: Side::Hidden, ..Default::default() };
        let as_text = Indicators { style: IndicatorStyle::Text, ..no_clock.clone() };
        assert_eq!(blocks(&no_clock, Side::Left), blocks(&as_text, Side::Left));
        assert_eq!(blocks(&no_clock, Side::Right), blocks(&as_text, Side::Right));
    }

    #[test]
    fn free_widths_dock_in_basso() {
        assert_eq!(free_widths(1512.0, DockPlace::Bottom { x0: 400.0, x1: 1112.0, top: 920.0 }), (372.0, 372.0));
    }

    #[test]
    fn free_widths_senza_dock() {
        let each = 1512.0 * (1.0 - 0.70) / 2.0 - 16.0 - 12.0;
        let (l, r) = free_widths(1512.0, DockPlace::Unknown);
        assert!((l - each).abs() < 0.01 && (r - each).abs() < 0.01);
        assert_eq!(free_widths(1512.0, DockPlace::Side), (1512.0 / 2.0 - 16.0, 1512.0 / 2.0 - 16.0));
    }

    #[test]
    fn free_widths_mai_negativo() {
        assert_eq!(free_widths(1000.0, DockPlace::Bottom { x0: -50.0, x1: 1100.0, top: 900.0 }), (0.0, 0.0));
    }

    #[test]
    fn fit_tutto() {
        assert_eq!(fit(&[50.0, 80.0], 200.0, 14.0), 2);
    }

    #[test]
    fn fit_omette_dal_dock() {
        assert_eq!(fit(&[50.0, 80.0, 90.0], 160.0, 14.0), 2);
        assert_eq!(fit(&[50.0, 500.0, 10.0], 160.0, 14.0), 1);
        assert_eq!(fit(&[], 0.0, 14.0), 0);
    }

    #[test]
    fn formati() {
        assert_eq!(fmt_cpu(23.4), "CPU 23%");
        assert_eq!(fmt_cpu(f32::NAN), "CPU 0%");
        assert_eq!(fmt_cpu(140.0), "CPU 100%");
        assert_eq!(fmt_ram(12_025_908_428, 17_179_869_184), "RAM 11,2 / 16 GB");
        assert_eq!(fmt_clock(9, 5), "09:05");
    }

    fn r(x: f64, y: f64, w: f64, h: f64) -> GRect {
        GRect { x, y, w, h }
    }

    #[test]
    fn dock_place_in_basso() {
        assert_eq!(dock_place(r(0., 0., 1512., 982.), Some(r(400., 920., 712., 58.)), true), DockPlace::Bottom { x0: 400.0, x1: 1112.0, top: 920.0 });
    }

    #[test]
    fn dock_place_secondo_schermo() {
        assert_eq!(dock_place(r(1512., 0., 1920., 1080.), Some(r(2000., 1010., 800., 64.)), true), DockPlace::Bottom { x0: 488.0, x1: 1288.0, top: 1010.0 });
    }

    #[test]
    fn dock_place_none() {
        assert_eq!(dock_place(r(0., 0., 1512., 982.), None, true), DockPlace::Unknown);
    }

    #[test]
    fn dock_place_laterale() {
        assert_eq!(dock_place(r(0., 0., 1512., 982.), Some(r(0., 200., 64., 500.)), false), DockPlace::Side);
    }

    #[test]
    fn center_y_non_esce_dallo_schermo() {
        let dock = DockPlace::Bottom { x0: 400.0, x1: 1100.0, top: 950.0 };
        assert_eq!(center_y(dock, 982.0, 20.0), 966.0);
        assert_eq!(center_y(dock, 982.0, 62.0), 982.0 - 4.0 - 31.0);
        assert_eq!(center_y(DockPlace::Unknown, 982.0, 20.0), 982.0 - 16.0 - 10.0);
    }

    #[test]
    fn ppp_minimo_per_il_testo() {
        assert_eq!(indicator_scale(0.5, 1.0, true), 1.0);
        assert_eq!(indicator_scale(0.5, 2.0, true), 0.5);
        assert_eq!(indicator_scale(0.5, 1.0, false), 0.5);
        assert_eq!(indicator_scale(1.0, 1.0, true), 1.0);
    }

    fn scr(x: f64, y: f64, w: f64, h: f64, vy: f64, vh: f64) -> (GRect, GRect) {
        (r(x, y, w, h), r(x, vy, w, vh))
    }

    #[test]
    fn choose_dock_visibile() {
        let s = [scr(0., 0., 1920., 1080., 31., 988.), scr(1920., 98., 1512., 982., 130., 950.)];
        let d = r(500., 1020., 900., 58.);
        assert_eq!(choose_dock(&s, Some(d), false), Some(DockInfo { screen: s[0].0, dock: Some(d), at_bottom: true }));
    }

    #[test]
    fn choose_dock_nascosto_usa_fascia_fissa() {
        // due schermi impilati: la barra nascosta del superiore "cade" nell'inferiore
        let s = [scr(0., 0., 1920., 1080., 31., 1049.), scr(0., 1080., 1920., 1080., 1080., 1080.)];
        let d = r(500., 1082., 900., 58.);
        let got = choose_dock(&s, Some(d), true).unwrap();
        assert_eq!(got.dock, None);
        assert_eq!(got.screen, s[0].0);
    }

    #[test]
    fn choose_dock_senza_permesso() {
        let s = [scr(0., 0., 1920., 1080., 0., 1080.), scr(1920., 0., 1512., 982., 31., 890.)];
        assert_eq!(choose_dock(&s, None, false), Some(DockInfo { screen: s[1].0, dock: None, at_bottom: true }));
        assert_eq!(choose_dock(&[], None, false), None);
    }
}
