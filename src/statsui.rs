//! Finestra "Le mie call": totale del periodo, grafico a barre e divisione per app.
//! Schede Giorno / Settimana / Mese / Anno, frecce per scorrere i periodi.

use crate::callstats::{fmt_duration, Bar, Period, PeriodKind, Report};
use crate::ui::{section, INK, MUTED, RED, RED_SOFT, TEXT};
use chrono::{NaiveDate, TimeZone};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Layout, Rect, RichText, Sense, Stroke, Vec2,
};
use std::time::Duration;

/// Barre dei giorni (o mesi) che devono ancora arrivare.
const DIM: Color32 = Color32::from_rgb(78, 44, 52);
const AXIS: Color32 = Color32::from_rgb(58, 46, 72);

/// Scelte dell'utente nella finestra, eseguite poi dall'app.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatsAction {
    Kind(PeriodKind),
    Prev,
    Next,
}

/// Disegna la finestra. `live` = c'è una call in corso: si ridisegna ogni 5 secondi.
pub fn stats_panel(
    ui: &mut egui::Ui,
    p: Period,
    today: NaiveDate,
    rep: &Report,
    recording: bool,
    live: bool,
) -> Option<StatsAction> {
    let mut act = None;
    egui::Frame::new()
        .fill(INK)
        .inner_margin(18.0)
        .show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            ui.label(RichText::new("Le mie call").size(22.0).strong().color(TEXT));
            ui.add_space(4.0);
            tabs(ui, p.kind(), &mut act);
            ui.add_space(6.0);

            // totale, nome del periodo e frecce
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(fmt_duration(rep.total_ms))
                        .size(30.0)
                        .strong()
                        .color(TEXT),
                );
                ui.label(RichText::new(p.label(today)).size(15.0).color(MUTED));
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui.add_enabled(p.can_go_next(today), arrow("›")).clicked() {
                        act = Some(StatsAction::Next);
                    }
                    if ui.add(arrow("‹")).clicked() {
                        act = Some(StatsAction::Prev);
                    }
                });
            });
            if let Some(avg) = rep.avg_workday_ms {
                ui.label(
                    RichText::new(format!("media {} al giorno lavorativo", fmt_duration(avg)))
                        .size(13.0)
                        .color(MUTED),
                );
            }
            if !recording {
                ui.label(
                    RichText::new("Registrazione disattivata")
                        .size(13.0)
                        .color(RED_SOFT),
                );
            }
            ui.add_space(8.0);

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    if p.kind() == PeriodKind::Day {
                        day_list(ui, rep);
                    } else {
                        chart(ui, &rep.bars);
                        if rep.per_app.is_empty() {
                            ui.add_space(6.0);
                            nessuna(ui);
                        }
                    }
                    if !rep.per_app.is_empty() {
                        per_app(ui, &rep.per_app);
                    }
                });
        });
    if live {
        ui.ctx().request_repaint_after(Duration::from_secs(5));
    }
    act
}

fn arrow(text: &str) -> egui::Button<'_> {
    egui::Button::new(RichText::new(text).size(20.0).color(TEXT))
        .fill(Color32::from_white_alpha(18))
        .corner_radius(7.0)
        .min_size(Vec2::new(34.0, 30.0))
}

fn tabs(ui: &mut egui::Ui, current: PeriodKind, act: &mut Option<StatsAction>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for (k, name) in [
            (PeriodKind::Day, "Giorno"),
            (PeriodKind::Week, "Settimana"),
            (PeriodKind::Month, "Mese"),
            (PeriodKind::Year, "Anno"),
        ] {
            let on = k == current;
            let text = RichText::new(name)
                .size(14.0)
                .color(if on { INK } else { TEXT });
            let fill = if on {
                RED
            } else {
                Color32::from_white_alpha(18)
            };
            if ui
                .add(
                    egui::Button::new(if on { text.strong() } else { text })
                        .fill(fill)
                        .corner_radius(7.0),
                )
                .clicked()
                && !on
            {
                *act = Some(StatsAction::Kind(k));
            }
        }
    });
}

fn nessuna(ui: &mut egui::Ui) {
    ui.label(RichText::new("Nessuna call").italics().color(MUTED));
}

/// Grafico a barre disegnato a mano; il suggerimento di ogni barra dice giorno e durata.
fn chart(ui: &mut egui::Ui, bars: &[Bar]) {
    if bars.is_empty() {
        return;
    }
    let (plot_h, label_h) = (150.0, 18.0);
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, plot_h + label_h), Sense::hover());
    let bottom = rect.top() + plot_h;
    let max = bars.iter().map(|b| b.ms).fold(0.0, f64::max);
    let slot = w / bars.len() as f32;
    let gap = (slot * 0.22).clamp(2.0, 10.0);
    // con 28–31 barre le etichette si alternano, altrimenti si sovrappongono
    let every = if bars.len() > 20 { 2 } else { 1 };
    let painter = ui.painter().clone();
    painter.line_segment(
        [
            egui::pos2(rect.left(), bottom + 0.5),
            egui::pos2(rect.right(), bottom + 0.5),
        ],
        Stroke::new(1.0, AXIS),
    );
    let radius = CornerRadius {
        nw: 3,
        ne: 3,
        sw: 0,
        se: 0,
    };
    for (i, b) in bars.iter().enumerate() {
        let x0 = rect.left() + slot * i as f32;
        let (l, r) = (x0 + gap / 2.0, x0 + slot - gap / 2.0);
        let mut h = if max > 0.0 {
            (b.ms / max) as f32 * (plot_h - 4.0)
        } else {
            0.0
        };
        if b.ms > 0.0 || b.future {
            // una barra minima resta visibile: le future si vedono più scure
            h = h.max(3.0);
        }
        if h > 0.0 {
            painter.rect_filled(
                Rect::from_min_max(egui::pos2(l, bottom - h), egui::pos2(r, bottom)),
                radius,
                if b.future { DIM } else { RED },
            );
        }
        if i % every == 0 {
            painter.text(
                egui::pos2((l + r) / 2.0, bottom + 4.0),
                Align2::CENTER_TOP,
                &b.label,
                FontId::proportional(11.0),
                MUTED,
            );
        }
        let area = Rect::from_min_max(
            egui::pos2(x0, rect.top()),
            egui::pos2(x0 + slot, rect.bottom()),
        );
        ui.interact(area, ui.id().with(("barra", i)), Sense::hover())
            .on_hover_text(&b.tooltip);
    }
}

/// Ora locale "HH:MM" di un istante in millisecondi Unix.
fn hhmm(ms: f64) -> String {
    chrono::Local
        .timestamp_millis_opt(ms as i64)
        .single()
        .map(|t| t.format("%H:%M").to_string())
        .unwrap_or_default()
}

fn day_list(ui: &mut egui::Ui, rep: &Report) {
    if rep.calls.is_empty() {
        nessuna(ui);
        return;
    }
    egui::Grid::new("call del giorno")
        .num_columns(3)
        .spacing([18.0, 8.0])
        .show(ui, |ui| {
            for c in &rep.calls {
                ui.label(
                    RichText::new(format!("{}–{}", hhmm(c.start_ms), hhmm(c.end_ms)))
                        .size(14.0)
                        .color(TEXT),
                );
                ui.label(RichText::new(&c.app).size(14.0).color(TEXT));
                ui.label(
                    RichText::new(fmt_duration(c.end_ms - c.start_ms))
                        .size(14.0)
                        .color(MUTED),
                );
                ui.end_row();
            }
        });
}

/// Una riga per app; la barretta è proporzionale alla prima (la più lunga).
fn per_app(ui: &mut egui::Ui, apps: &[(String, f64)]) {
    section(ui, "Per app");
    let first = apps.first().map(|a| a.1).unwrap_or(0.0);
    let (name_w, dur_w) = (130.0, 110.0);
    for (app, ms) in apps {
        ui.horizontal(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(name_w, 20.0),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.set_width(name_w);
                    ui.add(egui::Label::new(RichText::new(app).size(14.0).color(TEXT)).truncate());
                },
            );
            let bar_w = (ui.available_width() - dur_w).max(20.0);
            let (r, _) = ui.allocate_exact_size(Vec2::new(bar_w, 20.0), Sense::hover());
            let frac = if first > 0.0 {
                (*ms / first) as f32
            } else {
                0.0
            };
            let len = (bar_w * frac).max(3.0);
            let bar = Rect::from_min_size(
                egui::pos2(r.left(), r.center().y - 4.0),
                Vec2::new(len, 8.0),
            );
            ui.painter().rect_filled(bar, 4.0, RED);
            ui.label(RichText::new(fmt_duration(*ms)).size(13.0).color(MUTED));
        });
    }
}
