//! Menu a comparsa dell'icona nella barra di sistema (Windows), al posto del menu nativo:
//! intestazione con lo stato, chi è in call, azioni rapide e sottopagine per caffè e sfondi.
//! Ogni riga ha un'altezza fissa, così la finestra si dimensiona prima del primo disegno.

use crate::theme::{self, ic};
use crate::tray::Action;
use crate::ui::{Page, TrayPage, UiAction, View};
use egui::{Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};

pub const WIDTH: f64 = 320.0;
const PAD: f32 = 6.0;
const HEADER: f32 = 64.0;
const ROW: f32 = 36.0;
const SEP: f32 = 9.0;
const PERSON: f32 = 30.0;
const FOOTER: f32 = 30.0;
const BACK: f32 = 44.0;

/// Righe del menu principale, nell'ordine in cui compaiono.
enum Line {
    Item { icon: &'static str, label: String, trail: Option<String>, chevron: bool, color: Color32, action: Act },
    Toggle { label: &'static str, on: bool, action: Act },
    Sep,
}

#[derive(Clone)]
enum Act {
    Menu(Action),
    Ui(UiAction),
    Page(TrayPage),
}

fn lines(v: &View) -> Vec<Line> {
    let mut out = Vec::new();
    let item = |icon, label: &str, action| Line::Item { icon, label: label.to_string(), trail: None, chevron: false, color: theme::TEXT, action };
    match *v.tray_page {
        TrayPage::Main => {
            match &v.office.my_call {
                None => out.push(item(ic::PHONE, "Sto entrando in call", Act::Menu(Action::StartCall))),
                Some(_) => out.push(Line::Item { icon: ic::PHONE, label: "Termina la mia call".into(), trail: None, chevron: false, color: theme::RED_TEXT, action: Act::Menu(Action::EndCall) }),
            }
            match &v.office.coffee {
                Some(c) if c.mine => out.push(item(ic::COFFEE, "Termina la pausa caffè", Act::Menu(Action::EndCoffee))),
                Some(c) => out.push(Line::Item { icon: ic::COFFEE, label: format!("Pausa caffè di {}", c.by), trail: None, chevron: false, color: theme::TEXT, action: Act::Menu(Action::ShowCoffee) }),
                None => out.push(Line::Item { icon: ic::COFFEE, label: "Pausa caffè".into(), trail: None, chevron: true, color: theme::TEXT, action: Act::Page(TrayPage::Coffee) }),
            }
            if !v.office.others().is_empty() {
                out.push(item(ic::PEOPLE, "Mostra chi è in call", Act::Menu(Action::ShowCalls)));
            }
            out.push(Line::Sep);
            let wp = if !v.cfg.wallpaper_enabled {
                "Spenti".to_string()
            } else if v.cfg.paused {
                "In pausa".to_string()
            } else {
                v.catalog.iter().find(|w| w.id == v.cfg.wallpaper).map(|w| w.name.clone()).unwrap_or_default()
            };
            out.push(Line::Item { icon: ic::PICTURE, label: "Sfondi".into(), trail: Some(wp), chevron: true, color: theme::TEXT, action: Act::Page(TrayPage::Wallpaper) });
            out.push(item(ic::CHART, "Le mie call", Act::Menu(Action::OpenCallStats)));
            out.push(item(ic::GEAR, "Impostazioni", Act::Ui(UiAction::OpenSettings(Page::Home))));
            out.push(Line::Sep);
            if crate::update::repo().is_some() {
                use crate::update::UpdateCmd;
                for (text, enabled, cmd) in v.update.menu_items() {
                    if let (Some(c), true) = (cmd, enabled) {
                        let (icon, color, a) = match c {
                            UpdateCmd::Install => (ic::DOWNLOAD, theme::ACCENT_TEXT, Action::InstallUpdate),
                            UpdateCmd::Check => (ic::SYNC, theme::TEXT, Action::CheckUpdates),
                            UpdateCmd::OpenPage => (ic::OPEN, theme::TEXT, Action::OpenUpdate),
                        };
                        out.push(Line::Item { icon, label: text, trail: None, chevron: false, color, action: Act::Menu(a) });
                    }
                }
            }
            out.push(item(ic::POWER, "Esci", Act::Menu(Action::Quit)));
        }
        TrayPage::Coffee => {
            for (label, m) in [("Adesso", None), ("Tra 5 minuti", Some(5)), ("Tra 10 minuti", Some(10)), ("Tra 15 minuti", Some(15))] {
                out.push(item(if m.is_none() { ic::COFFEE } else { ic::CLOCK }, label, Act::Menu(Action::Coffee(m))));
            }
        }
        TrayPage::Wallpaper => {
            out.push(Line::Toggle { label: "Sfondi attivi", on: v.cfg.wallpaper_enabled, action: Act::Menu(Action::ToggleWallpaper) });
            if v.cfg.wallpaper_enabled {
                out.push(Line::Sep);
                for (w, first_still) in crate::tray::with_sections(v.catalog) {
                    if first_still {
                        out.push(Line::Sep);
                        out.push(Line::Item {
                            icon: "",
                            label: "Fissi — nessun consumo".into(),
                            trail: None,
                            chevron: false,
                            color: theme::TEXT_3,
                            action: Act::Page(TrayPage::Wallpaper),
                        });
                    }
                    let sel = w.id == v.cfg.wallpaper;
                    out.push(Line::Item {
                        icon: if sel { ic::CHECK } else { "" },
                        label: w.name.clone(),
                        trail: None,
                        chevron: false,
                        color: if sel { theme::TEXT } else { theme::TEXT_2 },
                        action: Act::Menu(Action::SetWallpaper(w.id.clone())),
                    });
                }
                out.push(Line::Sep);
                out.push(Line::Toggle { label: "In pausa", on: v.cfg.paused, action: Act::Menu(Action::TogglePause) });
            }
            out.push(Line::Sep);
            out.push(item(ic::SLIDERS, "Altre opzioni…", Act::Ui(UiAction::OpenSettings(Page::Wallpaper))));
        }
    }
    out
}

fn line_h(l: &Line) -> f32 {
    match l {
        Line::Sep => SEP,
        _ => ROW,
    }
}

/// Altezza della finestra per lo stato attuale.
pub fn height(v: &View) -> f64 {
    let body: f32 = lines(v).iter().map(line_h).sum();
    let top = match *v.tray_page {
        TrayPage::Main => {
            let n = v.office.others().len().min(4);
            HEADER + if n > 0 { PERSON * n as f32 + 12.0 } else { 0.0 } + SEP + FOOTER
        }
        _ => BACK + SEP,
    };
    (PAD * 2.0 + top + body) as f64
}

pub fn popup(ui: &mut egui::Ui, v: &mut View) {
    let full = ui.max_rect();
    ui.painter().rect_stroke(full.shrink(0.5), theme::R_MD, Stroke::new(1.0, theme::BORDER), egui::StrokeKind::Inside);
    let w = full.width() - PAD * 2.0;
    let mut y = full.top() + PAD;
    let x = full.left() + PAD;
    let mut act: Option<Act> = None;

    match *v.tray_page {
        TrayPage::Main => {
            header(ui, Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, HEADER)), v, &mut act);
            y += HEADER;
            let others = v.office.others();
            if !others.is_empty() {
                let n = others.len().min(4);
                let r = Rect::from_min_size(Pos2::new(x + 6.0, y), Vec2::new(w - 12.0, PERSON * n as f32 + 4.0));
                ui.painter().rect_filled(r, theme::R_SM, theme::RED_BG.gamma_multiply(0.7));
                for (i, c) in others.iter().take(n).enumerate() {
                    let row = Rect::from_min_size(Pos2::new(r.left() + 8.0, r.top() + 2.0 + PERSON * i as f32), Vec2::new(r.width() - 16.0, PERSON));
                    let p = ui.painter();
                    theme::paint_avatar(p, Pos2::new(row.left() + 10.0, row.center().y), &c.user, 20.0);
                    p.text(Pos2::new(row.left() + 28.0, row.center().y), egui::Align2::LEFT_CENTER, &c.user, FontId::new(13.0, theme::semibold_family()), theme::TEXT);
                    let secs = ((crate::config::now_ms() - c.start_ms) / 1000.0).max(0.0) as u64;
                    let t = if secs >= 3600 { format!("{}:{:02}:{:02}", secs / 3600, secs / 60 % 60, secs % 60) } else { format!("{:02}:{:02}", secs / 60, secs % 60) };
                    p.text(Pos2::new(row.right(), row.center().y), egui::Align2::RIGHT_CENTER, t, FontId::proportional(12.5), theme::RED_TEXT);
                    if let Some(app) = &c.app {
                        let name_w = p.layout_no_wrap(c.user.clone(), FontId::new(13.0, theme::semibold_family()), theme::TEXT).size().x;
                        p.text(Pos2::new(row.left() + 36.0 + name_w, row.center().y), egui::Align2::LEFT_CENTER, app, FontId::proportional(12.0), theme::TEXT_3);
                    }
                }
                y += PERSON * n as f32 + 12.0;
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
            }
            sep(ui, x, y, w);
            y += SEP;
        }
        TrayPage::Coffee | TrayPage::Wallpaper => {
            let title = if *v.tray_page == TrayPage::Coffee { "Pausa caffè" } else { "Sfondi" };
            let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, BACK));
            let back = Rect::from_min_size(Pos2::new(r.left() + 4.0, r.center().y - 15.0), Vec2::splat(30.0));
            let resp = ui.interact(back, ui.id().with("indietro"), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(back, theme::R_SM, theme::HOVER);
            }
            theme::paint_icon(ui.painter(), back.center(), ic::BACK, 13.0, theme::TEXT);
            if resp.clicked() {
                act = Some(Act::Page(TrayPage::Main));
            }
            ui.painter().text(Pos2::new(back.right() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, title, FontId::new(14.0, theme::semibold_family()), theme::TEXT);
            y += BACK;
            sep(ui, x, y, w);
            y += SEP;
        }
    }

    for (i, l) in lines(v).into_iter().enumerate() {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, line_h(&l)));
        match l {
            Line::Sep => sep(ui, x, y, w),
            Line::Item { icon, label, trail, chevron, color, action } => {
                let resp = ui.interact(r, ui.id().with(("voce", i)), Sense::click());
                let p = ui.painter();
                if resp.hovered() {
                    p.rect_filled(r, theme::R_SM, theme::HOVER);
                }
                if !icon.is_empty() {
                    theme::paint_icon(p, Pos2::new(r.left() + 20.0, r.center().y), icon, 14.0, if color == theme::TEXT { theme::TEXT_2 } else { color });
                }
                let lx = if theme::has_icons() { r.left() + 42.0 } else { r.left() + 14.0 };
                p.text(Pos2::new(lx, r.center().y), egui::Align2::LEFT_CENTER, &label, FontId::proportional(14.0), color);
                let mut rx = r.right() - 12.0;
                if chevron {
                    theme::paint_icon(p, Pos2::new(rx - 4.0, r.center().y), ic::CHEVRON_R, 10.0, theme::TEXT_3);
                    rx -= 20.0;
                }
                if let Some(t) = trail {
                    p.text(Pos2::new(rx, r.center().y), egui::Align2::RIGHT_CENTER, t, FontId::proportional(12.5), theme::TEXT_3);
                }
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                    act = Some(action);
                }
            }
            Line::Toggle { label, on, action } => {
                let resp = ui.interact(r, ui.id().with(("interruttore", i)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, theme::R_SM, theme::HOVER);
                }
                ui.painter().text(Pos2::new(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, label, FontId::proportional(14.0), theme::TEXT);
                let sw = Rect::from_min_size(Pos2::new(r.right() - 52.0, r.center().y - 10.0), Vec2::new(40.0, 20.0));
                let mut val = on;
                let changed = ui.scope_builder(egui::UiBuilder::new().max_rect(sw), |ui| theme::toggle(ui, &mut val).changed()).inner;
                if changed || resp.clicked() {
                    act = Some(action);
                }
            }
        }
        y += r.height();
    }

    if *v.tray_page == TrayPage::Main {
        let r = Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, FOOTER));
        let peers = v.peers.len();
        let txt = format!("Versione {} · {} {} in rete", env!("CARGO_PKG_VERSION"), peers, if peers == 1 { "collega" } else { "colleghi" });
        ui.painter().text(Pos2::new(r.left() + 14.0, r.center().y), egui::Align2::LEFT_CENTER, txt, FontId::proportional(11.5), theme::TEXT_3);
    }

    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        if *v.tray_page == TrayPage::Main {
            v.actions.push(UiAction::ClosePopup);
        } else {
            *v.tray_page = TrayPage::Main;
        }
    }

    match act {
        Some(Act::Page(p)) => *v.tray_page = p,
        Some(Act::Menu(a)) => {
            let keep = matches!(a, Action::ToggleWallpaper | Action::TogglePause | Action::SetWallpaper(_));
            v.actions.push(UiAction::Menu(a));
            if !keep {
                v.actions.push(UiAction::ClosePopup);
            }
        }
        Some(Act::Ui(a)) => {
            v.actions.push(a);
            v.actions.push(UiAction::ClosePopup);
        }
        None => {}
    }
}

fn sep(ui: &egui::Ui, x: f32, y: f32, w: f32) {
    ui.painter().hline((x + 8.0)..=(x + w - 8.0), y + SEP / 2.0, Stroke::new(1.0, theme::BORDER_SOFT));
}

fn header(ui: &mut egui::Ui, r: Rect, v: &mut View, act: &mut Option<Act>) {
    let tex = theme::mark_texture(ui.ctx());
    let logo = Rect::from_min_size(Pos2::new(r.left() + 10.0, r.center().y - 17.0), Vec2::splat(34.0));
    ui.painter().add(egui::Shape::Rect(egui::epaint::RectShape::filled(logo, 8.0, Color32::WHITE).with_texture(tex.id(), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)))));
    let tx = logo.right() + 12.0;
    let p = ui.painter();
    p.text(Pos2::new(tx, r.center().y - 9.0), egui::Align2::LEFT_CENTER, "DevLogica Tools", FontId::new(14.5, theme::semibold_family()), theme::TEXT);
    let others = v.office.others().len();
    let (status, color) = if v.office.my_call.is_some() {
        ("Sei in call".to_string(), theme::RED)
    } else if others == 1 {
        ("1 collega in call".to_string(), theme::RED)
    } else if others > 1 {
        (format!("{others} colleghi in call"), theme::RED)
    } else if v.office.coffee.is_some() {
        ("Pausa caffè in corso".to_string(), theme::AMBER)
    } else {
        (format!("Disponibile · {}", v.me), theme::GREEN)
    };
    p.circle_filled(Pos2::new(tx + 4.0, r.center().y + 10.0), 3.5, color);
    p.text(Pos2::new(tx + 13.0, r.center().y + 10.0), egui::Align2::LEFT_CENTER, status, FontId::proportional(12.5), theme::TEXT_2);
    // impostazioni
    let g = Rect::from_min_size(Pos2::new(r.right() - 40.0, r.center().y - 15.0), Vec2::splat(30.0));
    let resp = ui.interact(g, ui.id().with("ingranaggio"), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(g, theme::R_SM, theme::HOVER);
    }
    theme::paint_icon(ui.painter(), g.center(), ic::GEAR, 15.0, theme::TEXT_2);
    if resp.on_hover_text("Impostazioni").clicked() {
        *act = Some(Act::Ui(UiAction::OpenSettings(Page::Home)));
    }
}
