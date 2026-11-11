//! Finestra Impostazioni: barra laterale con le sezioni e, al centro, una panoramica a schede
//! (stato, ufficio, le mie call, sfondo, microfono) più le pagine di dettaglio.

use crate::callstats::fmt_duration;
use crate::config::{AlertStyle, Detection, Mode, Quality};
use crate::theme::{self, ic, Kind};
use crate::tray::Action;
use crate::ui::{date_field, DeleteForm, Page, UiAction, View};
use egui::{Align, Color32, Layout, Pos2, Rect, RichText, Sense, Stroke, UiBuilder, Vec2};

pub const SIDEBAR_W: f32 = 224.0;

pub fn settings(ui: &mut egui::Ui, v: &mut View) {
    let before = v.cfg.clone();
    let full = ui.max_rect();
    let side = Rect::from_min_size(full.min, Vec2::new(SIDEBAR_W, full.height()));
    ui.painter().rect_filled(side, 0.0, theme::BG_SIDE);
    ui.painter().vline(side.right(), side.y_range(), Stroke::new(1.0, theme::BORDER_SOFT));
    ui.scope_builder(UiBuilder::new().max_rect(side.shrink2(Vec2::new(12.0, 16.0))), |ui| sidebar(ui, v));

    let content = Rect::from_min_max(Pos2::new(side.right() + 1.0, full.top()), full.max);
    ui.scope_builder(UiBuilder::new().max_rect(content), |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).id_salt(("pagina", *v.page as u8)).show(ui, |ui| {
            egui::Frame::new().inner_margin(egui::Margin { left: 32, right: 32, top: 26, bottom: 28 }).show(ui, |ui| {
                ui.set_max_width(ui.available_width().min(820.0));
                match *v.page {
                    Page::Home => home(ui, v),
                    Page::Calls => calls_page(ui, v),
                    Page::Alerts => alerts_page(ui, v),
                    Page::Wallpaper => wallpaper_page(ui, v),
                    Page::MyCalls => my_calls_page(ui, v),
                    Page::Diagnostics => diagnostics_page(ui, v),
                    Page::About => about_page(ui, v),
                }
            });
        });
    });
    if *v.cfg != before {
        v.actions.push(UiAction::ConfigChanged);
    }
}

// ---------------------------------------------------------------- barra laterale

fn sidebar(ui: &mut egui::Ui, v: &mut View) {
    ui.horizontal(|ui| {
        ui.add_space(4.0);
        theme::mark(ui, 30.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(theme::strong("DevLogica Tools", 14.0));
            ui.label(RichText::new(format!("Versione {}", env!("CARGO_PKG_VERSION"))).size(11.5).color(theme::TEXT_3));
        });
    });
    ui.add_space(20.0);
    let update_badge = matches!(v.update, crate::update::UpdateState::Available(_));
    let items: &[(Page, &str, &str)] = &[
        (Page::Home, ic::HOME, "Panoramica"),
        (Page::Calls, ic::PHONE, "Call"),
        (Page::Alerts, ic::BELL, "Avvisi"),
        (Page::Wallpaper, ic::PICTURE, "Sfondi"),
        (Page::MyCalls, ic::CHART, "Le mie call"),
        (Page::Diagnostics, ic::DIAGNOSTIC, "Diagnostica"),
        (Page::About, ic::SYNC, "Aggiornamenti"),
    ];
    for (page, code, label) in items {
        let badge = *page == Page::About && update_badge;
        if nav_item(ui, code, label, *v.page == *page, badge) {
            *v.page = *page;
        }
    }

    // in fondo: chi sono e come sto
    let bottom = Rect::from_min_max(Pos2::new(ui.max_rect().left(), ui.max_rect().bottom() - 52.0), ui.max_rect().max);
    ui.scope_builder(UiBuilder::new().max_rect(bottom), |ui| {
        egui::Frame::new().fill(theme::SURFACE).corner_radius(theme::R_MD).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                theme::avatar(ui, &v.me, 30.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(theme::strong(&v.me, 13.5));
                    let (txt, col) = if v.office.my_call.is_some() { ("In call", theme::RED_TEXT) } else { ("Disponibile", theme::GREEN) };
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 5.0;
                        let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                        ui.painter().circle_filled(r.center(), 3.5, col);
                        ui.label(RichText::new(txt).size(12.0).color(theme::TEXT_2));
                    });
                });
            });
        });
    });
}

fn nav_item(ui: &mut egui::Ui, code: &str, label: &str, selected: bool, badge: bool) -> bool {
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 36.0), Sense::click());
    let p = ui.painter();
    if selected {
        p.rect_filled(rect, theme::R_SM, theme::SURFACE_HI);
        let bar = Rect::from_center_size(Pos2::new(rect.left() + 1.5, rect.center().y), Vec2::new(3.0, 16.0));
        p.rect_filled(bar, 2.0, theme::ACCENT);
    } else if resp.hovered() {
        p.rect_filled(rect, theme::R_SM, Color32::from_rgb(30, 30, 35));
    }
    let fg = if selected { theme::TEXT } else { theme::TEXT_2 };
    theme::paint_icon(p, Pos2::new(rect.left() + 20.0, rect.center().y), code, 15.0, if selected { theme::ACCENT_TEXT } else { fg });
    let x = if theme::has_icons() { rect.left() + 40.0 } else { rect.left() + 14.0 };
    let font = if selected { egui::FontId::new(14.0, theme::semibold_family()) } else { egui::FontId::proportional(14.0) };
    p.text(Pos2::new(x, rect.center().y), egui::Align2::LEFT_CENTER, label, font, fg);
    if badge {
        p.circle_filled(Pos2::new(rect.right() - 14.0, rect.center().y), 4.0, theme::ACCENT);
    }
    ui.add_space(2.0);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked()
}

fn page_header(ui: &mut egui::Ui, title: &str, subtitle: &str) {
    ui.label(theme::strong(title, 24.0));
    if !subtitle.is_empty() {
        ui.add_space(-2.0);
        ui.label(RichText::new(subtitle).size(13.5).color(theme::TEXT_2));
    }
    ui.add_space(10.0);
}

// ---------------------------------------------------------------- panoramica

fn greeting() -> &'static str {
    use chrono::Timelike;
    match chrono::Local::now().hour() {
        5..=12 => "Buongiorno",
        13..=17 => "Buon pomeriggio",
        _ => "Buonasera",
    }
}

fn names_sentence(names: &[String]) -> String {
    match names {
        [] => String::new(),
        [a] => a.clone(),
        [rest @ .., last] => format!("{} e {last}", rest.join(", ")),
    }
}

fn home(ui: &mut egui::Ui, v: &mut View) {
    let others: Vec<_> = v.office.others();
    let names: Vec<String> = others.iter().map(|c| c.user.clone()).collect();
    let summary = match names.len() {
        0 => "Nessun collega è in call in questo momento.".to_string(),
        1 => format!("{} è in call.", names[0]),
        _ => format!("{} sono in call.", names_sentence(&names)),
    };
    let first = v.me.split_whitespace().next().unwrap_or("").to_string();
    page_header(ui, &format!("{}, {first}", greeting()), &summary);

    // ---- update disponibile ----
    if let crate::update::UpdateState::Available(r) = v.update {
        egui::Frame::new().fill(theme::ACCENT_BG).stroke(Stroke::new(1.0, Color32::from_rgb(76, 54, 110))).corner_radius(theme::R_MD).inner_margin(egui::Margin::symmetric(16, 12)).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(theme::icon(ic::DOWNLOAD, 16.0, theme::ACCENT_TEXT));
                ui.label(RichText::new(format!("È disponibile la versione {}", r.tag)).size(14.0).color(theme::TEXT));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if r.asset.is_some() && theme::button(ui, Kind::Primary, "Aggiorna ora").clicked() {
                        v.actions.push(UiAction::InstallUpdate);
                    }
                });
            });
        });
        ui.add_space(12.0);
    }

    // ---- il mio stato ----
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            theme::avatar(ui, &v.me, 46.0);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.label(theme::eyebrow("Il tuo stato"));
                match &v.office.my_call {
                    Some(m) => {
                        let start = v.office.calls.values().find(|c| c.mine).map(|c| c.start_ms);
                        let dur = start.map(|s| fmt_duration(crate::config::now_ms() - s)).unwrap_or_default();
                        ui.horizontal(|ui| {
                            theme::status_dot(ui, theme::RED, true);
                            ui.label(theme::strong(format!("In call da {dur}"), 18.0));
                        });
                        let how = if m.auto { "riconosciuta in automatico" } else { "avviata a mano" };
                        ui.label(RichText::new(format!("{} · {how}", m.app.clone().unwrap_or_else(|| "Manuale".into()))).size(12.5).color(theme::TEXT_2));
                    }
                    None => {
                        ui.horizontal(|ui| {
                            theme::status_dot(ui, theme::GREEN, false);
                            ui.label(theme::strong("Disponibile", 18.0));
                        });
                        let det = match v.cfg.detection {
                            Detection::Auto => "Le call vengono riconosciute in automatico",
                            Detection::Ask => "Ti chiedo conferma quando entri in call",
                            Detection::Off => "Riconoscimento automatico disattivato",
                        };
                        ui.label(RichText::new(det).size(12.5).color(theme::TEXT_2));
                    }
                }
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if v.office.my_call.is_some() {
                    if theme::button_icon(ui, Kind::Danger, Some(ic::PHONE), "Termina la call").clicked() {
                        v.actions.push(UiAction::Menu(Action::EndCall));
                    }
                } else if theme::button_icon(ui, Kind::Primary, Some(ic::PHONE), "Sto entrando in call").clicked() {
                    v.actions.push(UiAction::Menu(Action::StartCall));
                }
            });
        });
        // pausa caffè
        ui.add_space(6.0);
        let r = ui.available_rect_before_wrap();
        ui.painter().hline(r.left()..=r.right(), r.top(), Stroke::new(1.0, theme::BORDER_SOFT));
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(theme::icon(ic::COFFEE, 16.0, theme::AMBER));
            match &v.office.coffee {
                Some(c) if c.mine => {
                    let n = c.rsvp.values().filter(|a| a.as_str() == "yes").count();
                    ui.label(RichText::new(format!("Hai proposto una pausa caffè · {n} vengono")).size(13.5).color(theme::TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, Kind::Secondary, "Termina la pausa").clicked() {
                            v.actions.push(UiAction::Menu(Action::EndCoffee));
                        }
                    });
                }
                Some(c) => {
                    ui.label(RichText::new(format!("{} propone una pausa caffè", c.by)).size(13.5).color(theme::TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, Kind::Secondary, "Rispondi").clicked() {
                            v.actions.push(UiAction::Menu(Action::ShowCoffee));
                        }
                    });
                }
                None => {
                    ui.label(RichText::new("Pausa caffè").size(13.5).color(theme::TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for (label, m) in [("Tra 15 min", Some(15)), ("Tra 10 min", Some(10)), ("Tra 5 min", Some(5)), ("Adesso", None)] {
                            let kind = if m.is_none() { Kind::Amber } else { Kind::Secondary };
                            if theme::button(ui, kind, label).clicked() {
                                v.actions.push(UiAction::Menu(Action::Coffee(m)));
                            }
                        }
                    });
                }
            }
        });
    });
    ui.add_space(12.0);

    // ---- ufficio + le mie call ----
    let w = (ui.available_width() - 12.0) / 2.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        tile(ui, w, 196.0, |ui| office_tile(ui, v));
        tile(ui, w, 196.0, |ui| my_calls_tile(ui, v));
    });
    ui.add_space(12.0);
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        tile(ui, w, 150.0, |ui| wallpaper_tile(ui, v));
        tile(ui, w, 150.0, |ui| mic_tile(ui, v));
    });
}

fn tile(ui: &mut egui::Ui, w: f32, h: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(Vec2::new(w, h), Layout::top_down(Align::Min), |ui| {
        egui::Frame::new().fill(theme::SURFACE).stroke(Stroke::new(1.0, theme::BORDER_SOFT)).corner_radius(theme::R_MD).inner_margin(egui::Margin::same(16)).show(ui, |ui| {
            ui.set_width(w - 34.0);
            ui.set_min_height(h - 32.0);
            add(ui)
        });
    });
}

fn tile_head(ui: &mut egui::Ui, code: &str, title: &str, link: Option<&str>) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.label(theme::icon(code, 14.0, theme::TEXT_3));
        ui.label(theme::eyebrow(title));
        if let Some(l) = link {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let r = ui.add(egui::Label::new(RichText::new(l).size(12.5).color(theme::ACCENT_TEXT)).sense(Sense::click()));
                clicked = r.on_hover_cursor(egui::CursorIcon::PointingHand).clicked();
            });
        }
    });
    ui.add_space(4.0);
    clicked
}

fn office_tile(ui: &mut egui::Ui, v: &mut View) {
    if tile_head(ui, ic::PEOPLE, "Ufficio", Some("Dettagli")) {
        *v.page = Page::Diagnostics;
    }
    let n = v.peers.len();
    ui.horizontal(|ui| {
        ui.label(theme::strong(n.to_string(), 26.0));
        ui.label(RichText::new(if n == 1 { "collega in rete" } else { "colleghi in rete" }).size(13.0).color(theme::TEXT_2));
    });
    ui.add_space(4.0);
    let others = v.office.others();
    if others.is_empty() {
        ui.label(RichText::new("Nessuno è in call").size(13.0).color(theme::TEXT_3));
    }
    for c in others.iter().take(3) {
        ui.horizontal(|ui| {
            theme::avatar(ui, &c.user, 22.0);
            ui.label(RichText::new(&c.user).size(13.5).color(theme::TEXT));
            ui.label(RichText::new(c.app.clone().unwrap_or_default()).size(12.0).color(theme::TEXT_3));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                theme::chip(ui, &fmt_duration(crate::config::now_ms() - c.start_ms), theme::RED_TEXT, theme::RED_BG);
            });
        });
    }
    if others.len() > 3 {
        ui.label(RichText::new(format!("e altri {}", others.len() - 3)).size(12.0).color(theme::TEXT_3));
    }
}

fn my_calls_tile(ui: &mut egui::Ui, v: &mut View) {
    if tile_head(ui, ic::CHART, "Le mie call · settimana", Some("Statistiche")) {
        v.actions.push(UiAction::OpenCallStats);
    }
    let Some(rep) = v.week else {
        ui.label(RichText::new("Nessun dato").color(theme::TEXT_3));
        return;
    };
    ui.horizontal(|ui| {
        ui.label(theme::strong(fmt_duration(rep.total_ms), 26.0));
        ui.label(RichText::new(format!("oggi {}", fmt_duration(v.today_ms))).size(13.0).color(theme::TEXT_2));
    });
    // barre dei giorni
    let h = ui.available_height().max(60.0) - 4.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
    let n = rep.bars.len().max(1);
    let max = rep.bars.iter().map(|b| b.ms).fold(60.0 * 60_000.0, f64::max);
    let slot = rect.width() / n as f32;
    let label_h = 16.0;
    for (i, b) in rep.bars.iter().enumerate() {
        let x = rect.left() + slot * i as f32;
        let bw = (slot * 0.56).min(26.0);
        let cx = x + slot / 2.0;
        let full_h = rect.height() - label_h - 4.0;
        let track = Rect::from_min_max(Pos2::new(cx - bw / 2.0, rect.top()), Pos2::new(cx + bw / 2.0, rect.top() + full_h));
        ui.painter().rect_filled(track, 4.0, Color32::from_rgb(34, 34, 40));
        let bh = (b.ms / max) as f32 * full_h;
        if bh > 0.5 {
            let bar = Rect::from_min_max(Pos2::new(track.left(), track.bottom() - bh.max(4.0)), track.max);
            ui.painter().rect_filled(bar, 4.0, if b.future { Color32::from_rgb(52, 52, 60) } else { theme::ACCENT });
        }
        ui.painter().text(Pos2::new(cx, rect.bottom() - 2.0), egui::Align2::CENTER_BOTTOM, &b.label, egui::FontId::proportional(11.0), theme::TEXT_3);
    }
}

fn wallpaper_tile(ui: &mut egui::Ui, v: &mut View) {
    if tile_head(ui, ic::PICTURE, "Sfondo animato", Some("Cambia")) {
        *v.page = Page::Wallpaper;
    }
    let name = v.catalog.iter().find(|w| w.id == v.cfg.wallpaper).map(|w| w.name.clone()).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.label(theme::strong(if v.cfg.wallpaper_enabled { name } else { "Disattivato".into() }, 18.0));
            let state = if !v.cfg.wallpaper_enabled {
                "Si vede lo sfondo di sistema".to_string()
            } else if v.cfg.paused {
                "In pausa".to_string()
            } else {
                format!("{} fps · {}", v.cfg.fps(), match v.cfg.mode { Mode::Same => "stesso su ogni schermo", Mode::PerMonitor => "uno per schermo", Mode::Span => "esteso" })
            };
            ui.label(RichText::new(state).size(12.5).color(theme::TEXT_2));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let mut on = v.cfg.wallpaper_enabled;
            if theme::toggle(ui, &mut on).changed() {
                v.actions.push(UiAction::Menu(Action::ToggleWallpaper));
            }
        });
    });
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add_enabled_ui(v.cfg.wallpaper_enabled, |ui| {
            let (code, label) = if v.cfg.paused { (ic::PLAY, "Riprendi") } else { (ic::PAUSE, "Pausa") };
            if theme::button_icon(ui, Kind::Secondary, Some(code), label).clicked() {
                v.actions.push(UiAction::Menu(Action::TogglePause));
            }
        });
    });
}

fn mic_tile(ui: &mut egui::Ui, v: &mut View) {
    if tile_head(ui, ic::MIC, "Microfono adesso", Some("Diagnostica")) {
        *v.page = Page::Diagnostics;
    }
    if v.mic.is_empty() {
        ui.label(theme::strong("Libero", 18.0));
        ui.label(RichText::new("Nessuna app sta usando il microfono").size(12.5).color(theme::TEXT_2));
        return;
    }
    for m in v.mic.iter().take(3) {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&m.label).size(14.0).color(theme::TEXT));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| mic_chip(ui, v, m));
        });
    }
}

fn mic_chip(ui: &mut egui::Ui, v: &View, m: &crate::detect::MicUser) {
    if v.cfg.ignored_apps.contains(&m.id) {
        theme::chip(ui, "Ignorata", theme::TEXT_2, theme::SURFACE_HI);
    } else if m.known_call_app {
        theme::chip(ui, "App di call", theme::GREEN, theme::GREEN_BG);
    } else {
        theme::chip(ui, "Non riconosciuta", theme::AMBER, theme::AMBER_BG);
    }
}

// ---------------------------------------------------------------- pagine

fn calls_page(ui: &mut egui::Ui, v: &mut View) {
    page_header(ui, "Call", "Come ti presenti ai colleghi e come riconosco le tue call.");
    theme::group(ui, |g| {
        g.row(Some(ic::PERSON), "Il tuo nome", "È quello che vedono i colleghi negli avvisi.", |ui| {
            ui.add(egui::TextEdit::singleline(&mut v.cfg.name).hint_text("es. Emanuele").desired_width(220.0).margin(Vec2::new(10.0, 7.0)));
        });
    });
    theme::section_title(ui, "Riconoscimento");
    theme::group(ui, |g| {
        g.row(Some(ic::MIC), "Quando il microfono si apre", "Guardo quale app usa il microfono: non ascolto e non registro niente.", |ui| {
            theme::segmented(ui, &mut v.cfg.detection, &[(Detection::Auto, "Automatico"), (Detection::Ask, "Chiedi"), (Detection::Off, "Spento")]);
        });
        g.row(Some(ic::BOX), "App non riconosciute", "Considera call anche le app che non sono nell'elenco.", |ui| {
            theme::toggle(ui, &mut v.cfg.detect_unknown);
        });
        g.row(Some(ic::PHONE), "Call a mano", "Avvisa l'ufficio anche senza un'app riconosciuta.", |ui| {
            if v.office.my_call.is_some() {
                if theme::button(ui, Kind::Danger, "Termina la call").clicked() {
                    v.actions.push(UiAction::Menu(Action::EndCall));
                }
            } else if theme::button(ui, Kind::Secondary, "Sto entrando in call").clicked() {
                v.actions.push(UiAction::Menu(Action::StartCall));
            }
        });
    });
}

fn alerts_page(ui: &mut egui::Ui, v: &mut View) {
    page_header(ui, "Avvisi", "Cosa succede sul tuo schermo quando un collega entra in call.");
    let opts = [
        (AlertStyle::Full, "Bordo e riquadro", "Bordo rosso attorno agli schermi e riquadro in alto"),
        (AlertStyle::Panel, "Solo riquadro", "Il riquadro in alto, senza bordo"),
        (AlertStyle::Tray, "Solo icona", "Cambia colore solo l'icona nella barra"),
    ];
    let w = (ui.available_width() - 24.0) / 3.0;
    ui.horizontal_top(|ui| {
        ui.spacing_mut().item_spacing.x = 12.0;
        for (style, title, desc) in opts {
            let sel = v.cfg.alert_style == style;
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 168.0), Sense::click());
            let p = ui.painter();
            let stroke = if sel { Stroke::new(1.5, theme::ACCENT) } else if resp.hovered() { Stroke::new(1.0, theme::HOVER) } else { Stroke::new(1.0, theme::BORDER_SOFT) };
            p.rect(rect, theme::R_MD, theme::SURFACE, stroke, egui::StrokeKind::Inside);
            // schermo in miniatura
            let scr = Rect::from_min_size(rect.min + Vec2::new(14.0, 14.0), Vec2::new(rect.width() - 28.0, 70.0));
            p.rect_filled(scr, 4.0, Color32::from_rgb(14, 14, 18));
            if style == AlertStyle::Full {
                p.rect_stroke(scr, 4.0, Stroke::new(2.0, theme::RED), egui::StrokeKind::Inside);
            }
            if style != AlertStyle::Tray {
                let box_ = Rect::from_center_size(Pos2::new(scr.center().x, scr.top() + 16.0), Vec2::new(scr.width() * 0.5, 14.0));
                p.rect_filled(box_, 3.0, theme::SURFACE_HI);
                p.rect_filled(Rect::from_min_size(box_.min, Vec2::new(box_.width(), 2.0)), 1.0, theme::RED);
            }
            let tray = Rect::from_min_size(Pos2::new(scr.left(), scr.bottom() - 9.0), Vec2::new(scr.width(), 9.0));
            p.rect_filled(tray, egui::CornerRadius { nw: 0, ne: 0, sw: 4, se: 4 }, Color32::from_rgb(30, 30, 36));
            p.circle_filled(Pos2::new(tray.right() - 10.0, tray.center().y), 2.5, theme::RED);
            // testo
            let ty = scr.bottom() + 14.0;
            p.text(Pos2::new(rect.left() + 14.0, ty), egui::Align2::LEFT_TOP, title, egui::FontId::new(14.0, theme::semibold_family()), theme::TEXT);
            let g = p.layout(desc.to_string(), egui::FontId::proportional(12.0), theme::TEXT_2, rect.width() - 28.0);
            p.galley(Pos2::new(rect.left() + 14.0, ty + 22.0), g, theme::TEXT_2);
            if sel {
                let c = Pos2::new(rect.right() - 24.0, ty + 10.0);
                p.circle_filled(c, 9.0, theme::ACCENT);
                theme::paint_icon(p, c, ic::CHECK, 10.0, Color32::WHITE);
            }
            if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                v.cfg.alert_style = style;
            }
        }
    });
    theme::section_title(ui, "Suono");
    theme::group(ui, |g| {
        g.row(Some(ic::VOLUME), "Volume dell'avviso", "Due note morbide quando un collega entra in call o propone un caffè.", |ui| {
            if theme::button(ui, Kind::Secondary, "Prova").clicked() {
                v.actions.push(UiAction::TestSound);
            }
            ui.spacing_mut().slider_width = 150.0;
            ui.add(egui::Slider::new(&mut v.cfg.volume, 0.0..=1.0).show_value(false));
        });
    });
}

fn wallpaper_page(ui: &mut egui::Ui, v: &mut View) {
    page_header(ui, "Sfondi animati", "Disegnati dalla scheda grafica, dietro alle icone del desktop.");
    theme::group(ui, |g| {
        g.row(Some(ic::PICTURE), "Sfondi animati", "Spenti, resta lo sfondo di Windows.", |ui| {
            let mut on = v.cfg.wallpaper_enabled;
            if theme::toggle(ui, &mut on).changed() {
                v.actions.push(UiAction::Menu(Action::ToggleWallpaper));
            }
        });
    });
    ui.add_enabled_ui(v.cfg.wallpaper_enabled, |ui| {
        theme::section_title(ui, "Sfondo");
        wallpaper_gallery(ui, v);
        theme::section_title(ui, "Schermi e prestazioni");
        theme::group(ui, |g| {
            g.row(Some(ic::SLIDERS), "Con più schermi", "", |ui| {
                let mut m = v.cfg.mode;
                if theme::segmented(ui, &mut m, &[(Mode::Same, "Uguale"), (Mode::PerMonitor, "Per schermo"), (Mode::Span, "Esteso")]) {
                    v.actions.push(UiAction::Menu(Action::SetMode(m)));
                }
            });
            if v.cfg.mode == Mode::PerMonitor {
                for (i, mon) in v.monitors.iter().enumerate() {
                    let current = v.cfg.per_monitor.get(&mon.key).unwrap_or(&v.cfg.wallpaper).clone();
                    let title = format!("Schermo {}", i + 1);
                    g.row(None, &title, &mon.label, |ui| {
                        let name = v.catalog.iter().find(|w| w.id == current).map(|w| w.name.clone()).unwrap_or_default();
                        egui::ComboBox::from_id_salt(("schermo", i)).selected_text(name).width(200.0).show_ui(ui, |ui| {
                            for w in v.catalog {
                                if ui.selectable_label(w.id == current, &w.name).clicked() {
                                    v.actions.push(UiAction::Menu(Action::SetMonitorWallpaper { monitor: mon.key.clone(), wallpaper: w.id.clone() }));
                                }
                            }
                        });
                    });
                }
            }
            g.row(Some(ic::GAUGE), "Fluidità", "Meno fotogrammi, meno consumo.", |ui| {
                let mut f = v.cfg.fps();
                if theme::segmented(ui, &mut f, &[(15, "15"), (24, "24"), (30, "30"), (60, "60 fps")]) {
                    v.actions.push(UiAction::Menu(Action::SetFps(f)));
                }
            });
            g.row(Some(ic::CPU), "Risoluzione", "Automatica: al massimo 1440 righe, ideale anche sui 4K.", |ui| {
                let mut q = v.cfg.quality;
                if theme::segmented(ui, &mut q, &[(Quality::Auto, "Automatica"), (Quality::Full, "Piena"), (Quality::Low, "Ridotta")]) {
                    v.actions.push(UiAction::Menu(Action::SetQuality(q)));
                }
            });
            g.row(None, "Mostra il logo", "", |ui| {
                let mut on = v.cfg.show_logo;
                if theme::toggle(ui, &mut on).changed() {
                    v.actions.push(UiAction::Menu(Action::ToggleLogo));
                }
            });
            g.row(None, "In pausa", "L'animazione si ferma sull'ultimo fotogramma.", |ui| {
                let mut on = v.cfg.paused;
                if theme::toggle(ui, &mut on).changed() {
                    v.actions.push(UiAction::Menu(Action::TogglePause));
                }
            });
            #[cfg(windows)]
            g.row(None, "Pausa con app a schermo intero", "Giochi, presentazioni e video a schermo intero.", |ui| {
                let mut on = v.cfg.pause_on_fullscreen;
                if theme::toggle(ui, &mut on).changed() {
                    v.actions.push(UiAction::Menu(Action::ToggleFullscreenPause));
                }
            });
            g.row(Some(ic::FOLDER), "Sfondi personalizzati", "File .wgsl nella cartella degli sfondi.", |ui| {
                if theme::button(ui, Kind::Secondary, "Ricarica").clicked() {
                    v.actions.push(UiAction::Menu(Action::Reload));
                }
                if theme::button(ui, Kind::Secondary, "Apri cartella").clicked() {
                    v.actions.push(UiAction::Menu(Action::OpenFolder));
                }
            });
        });
        #[cfg(target_os = "macos")]
        indicators_section(ui, v);
    });
}

/// Galleria degli sfondi con le miniature (se l'app le ha preparate).
fn wallpaper_gallery(ui: &mut egui::Ui, v: &mut View) {
    let cols = 3;
    let gap = 12.0;
    let w = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
    let th = w * 9.0 / 16.0;
    for row in v.catalog.chunks(cols) {
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for wp in row {
                let sel = v.cfg.wallpaper == wp.id;
                let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, th + 40.0), Sense::click());
                let img_r = Rect::from_min_size(rect.min, Vec2::new(w, th));
                match v.thumbs.iter().find(|(id, _)| *id == wp.id).map(|(id, img)| thumb_texture(ui.ctx(), id, img)) {
                    Some(tex) => {
                        ui.painter().add(egui::Shape::Rect(egui::epaint::RectShape::filled(img_r, theme::R_MD, Color32::WHITE).with_texture(tex.id(), Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)))));
                    }
                    None => {
                        ui.painter().rect_filled(img_r, theme::R_MD, theme::SURFACE_HI);
                        theme::paint_icon(ui.painter(), img_r.center(), ic::PICTURE, 22.0, theme::TEXT_3);
                    }
                }
                let p = ui.painter();
                let stroke = if sel { Stroke::new(2.0, theme::ACCENT) } else if resp.hovered() { Stroke::new(1.0, theme::TEXT_3) } else { Stroke::new(1.0, theme::BORDER_SOFT) };
                p.rect_stroke(img_r, theme::R_MD, stroke, egui::StrokeKind::Inside);
                p.text(Pos2::new(rect.left() + 2.0, img_r.bottom() + 10.0), egui::Align2::LEFT_TOP, &wp.name, egui::FontId::new(13.5, if sel { theme::semibold_family() } else { egui::FontFamily::Proportional }), if sel { theme::TEXT } else { theme::TEXT_2 });
                if sel {
                    let c = Pos2::new(img_r.right() - 16.0, img_r.top() + 16.0);
                    p.circle_filled(c, 10.0, theme::ACCENT);
                    theme::paint_icon(p, c, ic::CHECK, 10.0, Color32::WHITE);
                }
                if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !sel {
                    v.actions.push(UiAction::Menu(Action::SetWallpaper(wp.id.clone())));
                }
            }
        });
    }
}

/// Texture di una miniatura, caricata una volta per finestra.
fn thumb_texture(ctx: &egui::Context, id: &str, img: &std::sync::Arc<egui::ColorImage>) -> egui::TextureHandle {
    let key = egui::Id::new(("miniatura", id, img.size));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return t;
    }
    let t = ctx.load_texture(format!("miniatura-{id}"), (**img).clone(), egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(key, t.clone()));
    t
}

fn my_calls_page(ui: &mut egui::Ui, v: &mut View) {
    page_header(ui, "Le mie call", "Lo storico resta solo su questo PC e non viene mai inviato ai colleghi.");
    theme::group(ui, |g| {
        g.row(Some(ic::CHART), "Statistiche", "Totale per giorno, settimana, mese e anno, con il tempo per app.", |ui| {
            if theme::button_icon(ui, Kind::Secondary, Some(ic::OPEN), "Apri").clicked() {
                v.actions.push(UiAction::OpenCallStats);
            }
        });
        g.row(Some(ic::CLOCK), "Registra le mie call", "Spento, lo storico già salvato resta consultabile.", |ui| {
            theme::toggle(ui, &mut v.cfg.record_calls);
        });
    });
    theme::section_title(ui, "Cancella lo storico");
    delete_form(ui, v);
}

fn delete_form(ui: &mut egui::Ui, v: &mut View) {
    let first = v.first_call_day;
    let f: &mut DeleteForm = v.delete;
    let actions = &mut v.actions;
    theme::group(ui, |g| {
        let in_conferma = f.confirm.is_some_and(|(n, _)| n > 0);
        g.block(|ui| {
            ui.add_enabled_ui(!in_conferma, |ui| {
                ui.horizontal(|ui| {
                    let mut cambiato = false;
                    ui.label(RichText::new("Dal").color(theme::TEXT_2));
                    cambiato |= date_field(ui, "cancella_dal", &mut f.from);
                    ui.label(RichText::new("al").color(theme::TEXT_2));
                    cambiato |= date_field(ui, "cancella_al", &mut f.to);
                    if ui.add_enabled(first.is_some(), egui::Button::new("Tutto")).clicked() {
                        if let Some(d) = first {
                            f.from = d;
                            f.to = chrono::Local::now().date_naive();
                            cambiato = true;
                        }
                    }
                    if f.from > f.to {
                        std::mem::swap(&mut f.from, &mut f.to);
                    }
                    if cambiato {
                        f.confirm = None;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::button(ui, Kind::Secondary, "Cancella…").clicked() {
                            f.open = true;
                            actions.push(UiAction::PreviewDelete);
                        }
                    });
                });
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Giorni interi, estremi compresi. Una call a cavallo di un estremo viene tagliata.").size(12.0).color(theme::TEXT_3));
            match f.confirm {
                Some((0, _)) => {
                    ui.add_space(6.0);
                    ui.label(RichText::new("Nessuna call in questo intervallo.").color(theme::TEXT_2));
                }
                Some((n, ms)) => {
                    ui.add_space(8.0);
                    egui::Frame::new().fill(theme::RED_BG).corner_radius(theme::R_SM).inner_margin(egui::Margin::symmetric(12, 10)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            ui.label(theme::icon(ic::WARNING, 15.0, theme::RED_TEXT));
                            ui.add(egui::Label::new(RichText::new(crate::callstats::confirm_text(n, ms, f.from, f.to)).color(theme::RED_TEXT)).wrap());
                        });
                        ui.horizontal(|ui| {
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if theme::button(ui, Kind::Danger, "Cancella definitivamente").clicked() {
                                    actions.push(UiAction::DeleteCalls);
                                }
                                if theme::button(ui, Kind::Ghost, "Annulla").clicked() {
                                    f.confirm = None;
                                    f.open = false;
                                }
                            });
                        });
                    });
                }
                None => {}
            }
        });
    });
}

fn diagnostics_page(ui: &mut egui::Ui, v: &mut View) {
    page_header(ui, "Diagnostica", "Cosa vede l'app adesso: utile se una call non viene riconosciuta.");
    theme::section_title(ui, "Microfono in uso adesso");
    theme::group(ui, |g| {
        if v.mic.is_empty() {
            g.row(Some(ic::MIC), "Nessuna app sta usando il microfono", "Apri questa pagina durante una call per vedere cosa viene rilevato.", |_| {});
        }
        for m in v.mic {
            let ignored = v.cfg.ignored_apps.contains(&m.id);
            let id = m.id.clone();
            let mut act = None;
            g.row(Some(ic::MIC), &m.label, &m.id, |ui| {
                if ignored {
                    if theme::button(ui, Kind::Secondary, "Non ignorare").clicked() {
                        act = Some(UiAction::Unignore(id.clone()));
                    }
                } else if theme::button(ui, Kind::Secondary, "Ignora").clicked() {
                    act = Some(UiAction::Ignore(id.clone()));
                }
                let (t, fg, bg) = if ignored {
                    ("Ignorata", theme::TEXT_2, theme::SURFACE_HI)
                } else if m.known_call_app {
                    ("App di call", theme::GREEN, theme::GREEN_BG)
                } else {
                    ("Non riconosciuta", theme::AMBER, theme::AMBER_BG)
                };
                theme::chip(ui, t, fg, bg);
            });
            if let Some(a) = act {
                v.actions.push(a);
            }
        }
    });
    if !v.cfg.ignored_apps.is_empty() {
        theme::section_title(ui, "App ignorate");
        let ignored = v.cfg.ignored_apps.clone();
        theme::group(ui, |g| {
            for id in ignored {
                let mut act = false;
                let name = std::path::Path::new(&id.replace('\\', "/")).file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| id.clone());
                g.row(None, &name, &id, |ui| {
                    act = theme::button(ui, Kind::Secondary, "Rimuovi").clicked();
                });
                if act {
                    v.actions.push(UiAction::Unignore(id));
                }
            }
        });
    }
    theme::section_title(ui, "Colleghi in rete");
    theme::group(ui, |g| {
        if v.peers.is_empty() {
            g.row(Some(ic::WIFI), "Nessun collega trovato per ora", "Serve la stessa rete e il permesso del firewall per le reti private.", |_| {});
        }
        let mut peers = v.peers.clone();
        peers.sort();
        for p in &peers {
            g.block(|ui| {
                ui.horizontal(|ui| {
                    theme::avatar(ui, p, 26.0);
                    ui.label(RichText::new(p).size(14.0).color(theme::TEXT));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if v.office.calls.values().any(|c| &c.user == p) {
                            theme::chip(ui, "In call", theme::RED_TEXT, theme::RED_BG);
                        } else {
                            theme::chip(ui, "Online", theme::GREEN, theme::GREEN_BG);
                        }
                    });
                });
            });
        }
    });
}

fn about_page(ui: &mut egui::Ui, v: &mut View) {
    page_header(ui, "Aggiornamenti", "");
    theme::card(ui, |ui| {
        ui.horizontal(|ui| {
            theme::mark(ui, 56.0);
            ui.add_space(6.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.label(theme::strong("DevLogica Tools", 18.0));
                ui.label(RichText::new(format!("Versione {}", env!("CARGO_PKG_VERSION"))).size(13.0).color(theme::TEXT_2));
                let status = match v.update {
                    crate::update::UpdateState::Available(r) => format!("Disponibile la versione {}", r.tag),
                    crate::update::UpdateState::Checking { .. } => "Controllo in corso…".into(),
                    crate::update::UpdateState::Downloading(_) => "Scaricamento…".into(),
                    crate::update::UpdateState::Installing(_) => "Installazione…".into(),
                    crate::update::UpdateState::Failed { reason, .. } => format!("Aggiornamento non riuscito: {reason}"),
                    _ => "Controllo automatico all'avvio e due volte al giorno".into(),
                };
                ui.label(RichText::new(status).size(12.5).color(theme::TEXT_3));
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                use crate::update::UpdateCmd;
                for (text, enabled, cmd) in v.update.menu_items().into_iter().rev() {
                    if let (Some(c), true) = (cmd, enabled) {
                        let kind = if c == UpdateCmd::Install { Kind::Primary } else { Kind::Secondary };
                        if theme::button(ui, kind, &text).clicked() {
                            v.actions.push(match c {
                                UpdateCmd::Check => UiAction::CheckUpdates,
                                UpdateCmd::Install => UiAction::InstallUpdate,
                                UpdateCmd::OpenPage => UiAction::OpenUpdate,
                            });
                        }
                    }
                }
            });
        });
    });
    theme::section_title(ui, "Sistema");
    theme::group(ui, |g| {
        g.row(Some(ic::POWER), "Avvia all'accesso", "L'app parte da sola quando accendi il PC.", |ui| {
            let mut on = v.autostart;
            if theme::toggle(ui, &mut on).changed() {
                v.actions.push(UiAction::Menu(Action::ToggleAutostart));
            }
        });
        g.row(Some(ic::FOLDER), "Log e impostazioni", "Cartella con log.txt, config.json e lo storico delle call.", |ui| {
            if theme::button(ui, Kind::Secondary, "Apri cartella").clicked() {
                v.actions.push(UiAction::OpenAppDir);
            }
        });
        g.row(Some(ic::POWER), "Esci da DevLogica Tools", "Gli avvisi dei colleghi non arriveranno più.", |ui| {
            if theme::button(ui, Kind::Danger, "Esci").clicked() {
                v.actions.push(UiAction::Menu(Action::Quit));
            }
        });
    });
}

/// Indicatori sullo sfondo: solo su macOS, dove il Dock lascia libere le zone ai lati.
#[cfg(target_os = "macos")]
fn indicators_section(ui: &mut egui::Ui, v: &mut View) {
    use crate::config::{IndicatorStyle, Side};
    theme::section_title(ui, "Indicatori sullo sfondo");
    let ax = v.ax_trusted;
    let mut open_ax = false;
    let ind = &mut v.cfg.indicators;
    theme::group(ui, |g| {
        g.row(None, "CPU, RAM, orologio e logo", "Ai lati del Dock, dentro lo sfondo animato.", |ui| {
            theme::toggle(ui, &mut ind.enabled);
        });
        g.row(None, "Stile", "", |ui| {
            theme::segmented(ui, &mut ind.style, &[(IndicatorStyle::Text, "Testo"), (IndicatorStyle::Pills, "Pillole"), (IndicatorStyle::BigClock, "Orologio grande")]);
        });
        for (label, side) in [("Logo", &mut ind.logo), ("Orologio", &mut ind.clock), ("RAM", &mut ind.ram), ("CPU", &mut ind.cpu)] {
            g.row(None, label, "", |ui| {
                theme::segmented(ui, side, &[(Side::Hidden, "Nascosto"), (Side::Left, "Sinistra"), (Side::Right, "Destra")]);
            });
        }
        let pct = |v: f64, _: std::ops::RangeInclusive<usize>| format!("{:.0}%", v * 100.0);
        g.row(None, "Dimensione dei testi", "", |ui| {
            ui.add(egui::Slider::new(&mut ind.text_scale, 0.75..=2.0).step_by(0.05).custom_formatter(pct));
        });
        g.row(None, "Dimensione del logo", "", |ui| {
            ui.add(egui::Slider::new(&mut ind.logo_scale, 0.75..=2.0).step_by(0.05).custom_formatter(pct));
        });
        let desc = if ax { "Consentita" } else { "Senza permesso lascio libera una fascia al centro." };
        g.row(None, "Posizione esatta del Dock", desc, |ui| {
            if !ax && theme::button(ui, Kind::Secondary, "Consenti…").clicked() {
                open_ax = true;
            }
        });
    });
    if open_ax {
        v.actions.push(UiAction::OpenAccessibility);
    }
}
