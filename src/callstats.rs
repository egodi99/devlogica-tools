//! Statistiche delle call: logica pura sulle date (periodi, divisione a mezzanotte, formati).
//! Fuso locale, settimana lunedì–domenica, giorni interi, nomi italiani senza locale di sistema.

use chrono::{DateTime, Datelike, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

const MESI: [&str; 12] = [
    "gennaio",
    "febbraio",
    "marzo",
    "aprile",
    "maggio",
    "giugno",
    "luglio",
    "agosto",
    "settembre",
    "ottobre",
    "novembre",
    "dicembre",
];
const GIORNI: [&str; 7] = [
    "lunedì",
    "martedì",
    "mercoledì",
    "giovedì",
    "venerdì",
    "sabato",
    "domenica",
];

/// Una call registrata: istanti in millisecondi Unix, app in primo piano e origine.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Segment {
    pub start_ms: f64,
    pub end_ms: f64,
    pub app: String,
    pub auto: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeriodKind {
    Day,
    Week,
    Month,
    Year,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Period {
    Day(NaiveDate),
    /// Il lunedì della settimana.
    Week(NaiveDate),
    Month(i32, u32),
    Year(i32),
}

impl Period {
    pub fn containing(kind: PeriodKind, d: NaiveDate) -> Period {
        match kind {
            PeriodKind::Day => Period::Day(d),
            PeriodKind::Week => {
                Period::Week(d - Duration::days(d.weekday().num_days_from_monday() as i64))
            }
            PeriodKind::Month => Period::Month(d.year(), d.month()),
            PeriodKind::Year => Period::Year(d.year()),
        }
    }

    pub fn kind(&self) -> PeriodKind {
        match self {
            Period::Day(_) => PeriodKind::Day,
            Period::Week(_) => PeriodKind::Week,
            Period::Month(..) => PeriodKind::Month,
            Period::Year(_) => PeriodKind::Year,
        }
    }

    pub fn first_day(&self) -> NaiveDate {
        match *self {
            Period::Day(d) | Period::Week(d) => d,
            Period::Month(y, m) => NaiveDate::from_ymd_opt(y, m, 1).expect("mese valido"),
            Period::Year(y) => NaiveDate::from_ymd_opt(y, 1, 1).expect("anno valido"),
        }
    }

    /// Ultimo giorno, compreso.
    pub fn last_day(&self) -> NaiveDate {
        match *self {
            Period::Day(d) => d,
            Period::Week(d) => d + Duration::days(6),
            Period::Month(..) | Period::Year(_) => self.next().first_day() - Duration::days(1),
        }
    }

    pub fn prev(&self) -> Period {
        match *self {
            Period::Day(d) => Period::Day(d - Duration::days(1)),
            Period::Week(d) => Period::Week(d - Duration::days(7)),
            Period::Month(y, 1) => Period::Month(y - 1, 12),
            Period::Month(y, m) => Period::Month(y, m - 1),
            Period::Year(y) => Period::Year(y - 1),
        }
    }

    pub fn next(&self) -> Period {
        match *self {
            Period::Day(d) => Period::Day(d + Duration::days(1)),
            Period::Week(d) => Period::Week(d + Duration::days(7)),
            Period::Month(y, 12) => Period::Month(y + 1, 1),
            Period::Month(y, m) => Period::Month(y, m + 1),
            Period::Year(y) => Period::Year(y + 1),
        }
    }

    /// Falso se il periodo successivo inizia dopo oggi.
    pub fn can_go_next(&self, today: NaiveDate) -> bool {
        self.next().first_day() <= today
    }

    pub fn label(&self, today: NaiveDate) -> String {
        match *self {
            Period::Day(d) if d == today => "oggi".to_string(),
            Period::Day(d) if d == today - Duration::days(1) => "ieri".to_string(),
            Period::Day(d) => format!(
                "{} {} {}",
                GIORNI[d.weekday().num_days_from_monday() as usize],
                giorno_mese(d),
                d.year()
            ),
            Period::Week(_) => {
                let (a, b) = (self.first_day(), self.last_day());
                if a.year() != b.year() {
                    format!(
                        "{} {} {} – {} {} {}",
                        num(a),
                        nome_mese(a),
                        a.year(),
                        num(b),
                        nome_mese(b),
                        b.year()
                    )
                } else if a.month() != b.month() {
                    format!("{} {} – {} {}", num(a), nome_mese(a), num(b), nome_mese(b))
                } else {
                    format!("{}–{} {}", num(a), num(b), nome_mese(a))
                }
            }
            Period::Month(y, m) => format!("{} {}", MESI[m as usize - 1], y),
            Period::Year(y) => y.to_string(),
        }
    }
}

/// Numero del giorno: il primo del mese è "1°".
fn num(d: NaiveDate) -> String {
    if d.day() == 1 {
        "1°".to_string()
    } else {
        d.day().to_string()
    }
}

fn nome_mese(d: NaiveDate) -> &'static str {
    MESI[d.month0() as usize]
}

fn giorno_mese(d: NaiveDate) -> String {
    format!("{} {}", num(d), nome_mese(d))
}

/// Data locale di un istante in millisecondi Unix.
pub fn local_date<Tz: TimeZone>(ms: f64, tz: &Tz) -> NaiveDate {
    let utc: DateTime<Utc> = Utc
        .timestamp_millis_opt(ms.floor() as i64)
        .single()
        .unwrap_or_default();
    utc.with_timezone(tz).date_naive()
}

/// Mezzanotte locale del giorno `d`, in millisecondi Unix. Se la mezzanotte non esiste
/// (salto dell'ora legale) vale il primo istante valido del giorno.
pub fn day_start_ms<Tz: TimeZone>(d: NaiveDate, tz: &Tz) -> f64 {
    let mut ora = NaiveTime::MIN;
    for _ in 0..48 {
        if let Some(dt) = tz.from_local_datetime(&d.and_time(ora)).earliest() {
            return dt.timestamp_millis() as f64;
        }
        ora += Duration::minutes(30);
    }
    // Irraggiungibile con fusi reali: ripiego su UTC.
    Utc.from_utc_datetime(&d.and_time(NaiveTime::MIN))
        .timestamp_millis() as f64
}

/// Millisecondi della call per ciascun giorno locale toccato. I confini sono istanti reali:
/// un cambio dell'ora legale durante la call non aggiunge e non toglie un'ora.
pub fn per_day<Tz: TimeZone>(seg: &Segment, tz: &Tz) -> Vec<(NaiveDate, f64)> {
    let mut out = Vec::new();
    if seg.end_ms <= seg.start_ms {
        return out;
    }
    let ultimo = local_date(seg.end_ms, tz);
    let mut d = local_date(seg.start_ms, tz);
    while d <= ultimo {
        let da = seg.start_ms.max(day_start_ms(d, tz));
        let a = seg.end_ms.min(day_start_ms(d + Duration::days(1), tz));
        if a > da {
            out.push((d, a - da));
        }
        d += Duration::days(1);
    }
    out
}

/// Giorni lunedì–venerdì tra `from` e `to`, estremi compresi.
pub fn workdays(from: NaiveDate, to: NaiveDate) -> u32 {
    let mut n = 0;
    let mut d = from;
    while d <= to {
        if d.weekday().num_days_from_monday() < 5 {
            n += 1;
        }
        d += Duration::days(1);
    }
    n
}

/// "2 h 10 min", "2 h", "45 min", "< 1 min" (sotto il minuto ma oltre zero), "0 min".
pub fn fmt_duration(ms: f64) -> String {
    if ms <= 0.0 {
        return "0 min".to_string();
    }
    if ms < 60_000.0 {
        return "< 1 min".to_string();
    }
    let min = (ms / 60_000.0).floor() as u64;
    match (min / 60, min % 60) {
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

/// Una barra del grafico: etichetta sull'asse, testo del suggerimento e durata.
#[derive(Clone, Debug, PartialEq)]
pub struct Bar {
    pub label: String,
    pub tooltip: String,
    pub ms: f64,
    pub future: bool,
}

/// Una call della scheda Giorno: la parte del tratto che cade in quel giorno.
#[derive(Clone, Debug, PartialEq)]
pub struct CallRow {
    pub start_ms: f64,
    pub end_ms: f64,
    pub app: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub total_ms: f64,
    /// Assente nella scheda Giorno o con 0 giorni lavorativi fino a oggi.
    pub avg_workday_ms: Option<f64>,
    /// Vuoto nella scheda Giorno.
    pub bars: Vec<Bar>,
    /// Solo nella scheda Giorno, dalla più recente.
    pub calls: Vec<CallRow>,
    /// Per durata decrescente.
    pub per_app: Vec<(String, f64)>,
}

const LETTERE_MESI: [&str; 12] = ["G", "F", "M", "A", "M", "G", "L", "A", "S", "O", "N", "D"];

fn abbreviato(nome: &str) -> String {
    nome.chars().take(3).collect()
}

pub fn report<Tz: TimeZone>(segs: &[Segment], p: Period, today: NaiveDate, tz: &Tz) -> Report {
    let (primo, ultimo) = (p.first_day(), p.last_day());
    // Millisecondi per giorno, solo dentro il periodo.
    let mut giorni: std::collections::BTreeMap<NaiveDate, f64> = Default::default();
    let mut per_app: Vec<(String, f64)> = Vec::new();
    let mut calls = Vec::new();
    for s in segs {
        for (d, ms) in per_day(s, tz) {
            if d < primo || d > ultimo {
                continue;
            }
            *giorni.entry(d).or_insert(0.0) += ms;
            match per_app.iter_mut().find(|(a, _)| *a == s.app) {
                Some(e) => e.1 += ms,
                None => per_app.push((s.app.clone(), ms)),
            }
            if let Period::Day(_) = p {
                calls.push(CallRow {
                    start_ms: s.start_ms.max(day_start_ms(d, tz)),
                    end_ms: s.end_ms.min(day_start_ms(d + Duration::days(1), tz)),
                    app: s.app.clone(),
                });
            }
        }
    }
    per_app.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    calls.sort_by(|a, b| b.start_ms.total_cmp(&a.start_ms));
    let total_ms: f64 = giorni.values().sum();
    let del_giorno = |d: NaiveDate| giorni.get(&d).copied().unwrap_or(0.0);

    let mut bars = Vec::new();
    match p {
        Period::Day(_) => {}
        Period::Week(_) | Period::Month(..) => {
            let mut d = primo;
            while d <= ultimo {
                let ms = del_giorno(d);
                let (label, nome) = if let Period::Week(_) = p {
                    let g = abbreviato(GIORNI[d.weekday().num_days_from_monday() as usize]);
                    (g.clone(), format!("{g} {}", d.day()))
                } else {
                    (
                        d.day().to_string(),
                        format!("{} {}", d.day(), abbreviato(nome_mese(d))),
                    )
                };
                bars.push(Bar {
                    label,
                    tooltip: format!("{nome}: {}", fmt_duration(ms)),
                    ms,
                    future: d > today,
                });
                d += Duration::days(1);
            }
        }
        Period::Year(y) => {
            for m in 1..=12u32 {
                let inizio = NaiveDate::from_ymd_opt(y, m, 1).expect("mese valido");
                let ms: f64 = giorni
                    .range(inizio..)
                    .take_while(|(d, _)| d.month() == m)
                    .map(|(_, v)| v)
                    .sum();
                bars.push(Bar {
                    label: LETTERE_MESI[m as usize - 1].to_string(),
                    tooltip: format!("{}: {}", MESI[m as usize - 1], fmt_duration(ms)),
                    ms,
                    future: inizio > today,
                });
            }
        }
    }

    let avg_workday_ms = if p.kind() == PeriodKind::Day {
        None
    } else {
        let n = if today < primo {
            0
        } else {
            workdays(primo, ultimo.min(today))
        };
        (n > 0).then(|| total_ms / n as f64)
    };
    Report {
        total_ms,
        avg_workday_ms,
        bars,
        calls,
        per_app,
    }
}

/// Testo di conferma della cancellazione: "Cancellare 37 call (21 h 40 min) dal 1° al 31
/// ottobre 2026?". Mese e anno si ripetono solo quando cambiano; "dall'8", "all'11".
pub fn confirm_text(calls: usize, ms: f64, from: NaiveDate, to: NaiveDate) -> String {
    let da = if from.year() != to.year() {
        format!("{} {}", giorno_mese(from), from.year())
    } else if from.month() != to.month() {
        giorno_mese(from)
    } else {
        num(from)
    };
    format!(
        "Cancellare {calls} call ({}) {}{da} {}{} {}?",
        fmt_duration(ms),
        prep(from, "dal "),
        prep(to, "al "),
        giorno_mese(to),
        to.year()
    )
}

/// "dal "/"al " davanti al giorno; davanti a 8 e 11 si elide: "dall'", "all'".
fn prep(d: NaiveDate, base: &str) -> String {
    if matches!(d.day(), 8 | 11) {
        format!("{}l'", base.trim_end())
    } else {
        base.to_string()
    }
}

/// Risultato di `cut_range`: i tratti che restano e quanto è stato tolto.
#[derive(Clone, Debug, PartialEq)]
pub struct Cut {
    pub kept: Vec<Segment>,
    /// Tratti toccati dal taglio.
    pub calls: usize,
    pub removed_ms: f64,
}

/// Toglie dai tratti la parte tra l'inizio di `from` e la fine di `to` (giorni compresi).
/// Un tratto può restare intero, accorciarsi o dividersi in due.
pub fn cut_range<Tz: TimeZone>(segs: &[Segment], from: NaiveDate, to: NaiveDate, tz: &Tz) -> Cut {
    let da = day_start_ms(from, tz);
    let a = day_start_ms(to + Duration::days(1), tz);
    let mut cut = Cut {
        kept: Vec::new(),
        calls: 0,
        removed_ms: 0.0,
    };
    for s in segs {
        let tolto = s.end_ms.min(a) - s.start_ms.max(da);
        if tolto <= 0.0 {
            cut.kept.push(s.clone());
            continue;
        }
        cut.calls += 1;
        cut.removed_ms += tolto;
        if s.start_ms < da {
            cut.kept.push(Segment {
                end_ms: da,
                ..s.clone()
            });
        }
        if s.end_ms > a {
            cut.kept.push(Segment {
                start_ms: a,
                ..s.clone()
            });
        }
    }
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::FixedOffset;

    fn tz() -> FixedOffset {
        FixedOffset::east_opt(3600).unwrap()
    }

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    /// Millisecondi Unix di quell'ora locale.
    fn ms(y: i32, m: u32, d: u32, h: u32, min: u32) -> f64 {
        let dt = tz()
            .from_local_datetime(&date(y, m, d).and_hms_opt(h, min, 0).unwrap())
            .unwrap();
        dt.timestamp_millis() as f64
    }

    fn seg(start_ms: f64, end_ms: f64) -> Segment {
        Segment {
            start_ms,
            end_ms,
            app: "Teams".into(),
            auto: true,
        }
    }

    const MIN: f64 = 60_000.0;

    #[test]
    fn formati_durata() {
        assert_eq!(fmt_duration(0.0), "0 min");
        assert_eq!(fmt_duration(30_000.0), "< 1 min");
        assert_eq!(fmt_duration(45.0 * MIN), "45 min");
        assert_eq!(fmt_duration(130.0 * MIN), "2 h 10 min");
        assert_eq!(fmt_duration(120.0 * MIN), "2 h");
    }

    #[test]
    fn divide_a_mezzanotte() {
        let s = seg(ms(2026, 11, 10, 23, 30), ms(2026, 11, 11, 1, 15));
        assert_eq!(
            per_day(&s, &tz()),
            vec![
                (date(2026, 11, 10), 30.0 * MIN),
                (date(2026, 11, 11), 75.0 * MIN)
            ]
        );
    }

    #[test]
    fn call_nello_stesso_giorno() {
        let s = seg(ms(2026, 11, 11, 9, 0), ms(2026, 11, 11, 10, 0));
        assert_eq!(per_day(&s, &tz()), vec![(date(2026, 11, 11), 60.0 * MIN)]);
    }

    #[test]
    fn call_che_finisce_a_mezzanotte_non_crea_giorno_vuoto() {
        let s = seg(ms(2026, 11, 10, 23, 0), ms(2026, 11, 11, 0, 0));
        assert_eq!(per_day(&s, &tz()), vec![(date(2026, 11, 10), 60.0 * MIN)]);
    }

    #[test]
    fn periodi_contenenti() {
        let w = Period::containing(PeriodKind::Week, date(2026, 11, 11));
        assert_eq!(w, Period::Week(date(2026, 11, 9)));
        assert_eq!(w.last_day(), date(2026, 11, 15));
        let m = Period::containing(PeriodKind::Month, date(2026, 11, 11));
        assert_eq!(m, Period::Month(2026, 11));
        assert_eq!(m.last_day(), date(2026, 11, 30));
        assert_eq!(Period::Month(2028, 2).last_day(), date(2028, 2, 29));
        assert_eq!(Period::Year(2026).last_day(), date(2026, 12, 31));
    }

    #[test]
    fn navigazione() {
        assert_eq!(
            Period::Week(date(2026, 11, 9)).prev(),
            Period::Week(date(2026, 11, 2))
        );
        assert_eq!(Period::Month(2026, 1).prev(), Period::Month(2025, 12));
        assert_eq!(Period::Year(2026).next(), Period::Year(2027));
        let oggi = date(2026, 11, 11);
        assert!(!Period::Week(date(2026, 11, 9)).can_go_next(oggi));
        assert!(Period::Week(date(2026, 11, 2)).can_go_next(oggi));
    }

    #[test]
    fn etichette() {
        let oggi = date(2026, 11, 11);
        assert_eq!(Period::Day(date(2026, 11, 11)).label(oggi), "oggi");
        assert_eq!(Period::Day(date(2026, 11, 10)).label(oggi), "ieri");
        assert_eq!(
            Period::Day(date(2026, 11, 3)).label(oggi),
            "martedì 3 novembre 2026"
        );
        assert_eq!(
            Period::Day(date(2026, 11, 1)).label(oggi),
            "domenica 1° novembre 2026"
        );
        assert_eq!(Period::Week(date(2026, 11, 9)).label(oggi), "9–15 novembre");
        assert_eq!(Period::Month(2026, 11).label(oggi), "novembre 2026");
        assert_eq!(Period::Year(2026).label(oggi), "2026");
    }

    #[test]
    fn settimana_a_cavallo_dell_anno() {
        let oggi = date(2026, 11, 11);
        assert_eq!(
            Period::Week(date(2025, 12, 29)).label(oggi),
            "29 dicembre 2025 – 4 gennaio 2026"
        );
        assert_eq!(
            Period::Week(date(2026, 10, 26)).label(oggi),
            "26 ottobre – 1° novembre"
        );
    }

    #[test]
    fn giorni_lavorativi() {
        assert_eq!(workdays(date(2026, 11, 9), date(2026, 11, 15)), 5);
        assert_eq!(workdays(date(2026, 11, 9), date(2026, 11, 11)), 3);
        assert_eq!(workdays(date(2026, 11, 14), date(2026, 11, 15)), 0);
    }

    fn seg_app(start_ms: f64, end_ms: f64, app: &str) -> Segment {
        Segment {
            start_ms,
            end_ms,
            app: app.into(),
            auto: true,
        }
    }

    const H: f64 = 3_600_000.0;

    #[test]
    fn report_settimana() {
        let segs = vec![
            seg(ms(2026, 11, 9, 9, 0), ms(2026, 11, 9, 10, 0)),
            seg(ms(2026, 11, 11, 9, 0), ms(2026, 11, 11, 11, 0)),
            seg(ms(2026, 11, 8, 9, 0), ms(2026, 11, 8, 9, 30)),
        ];
        let r = report(
            &segs,
            Period::Week(date(2026, 11, 9)),
            date(2026, 11, 11),
            &tz(),
        );
        assert_eq!(r.total_ms, 3.0 * H);
        let etichette: Vec<&str> = r.bars.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(etichette, ["lun", "mar", "mer", "gio", "ven", "sab", "dom"]);
        let valori: Vec<f64> = r.bars.iter().map(|b| b.ms).collect();
        assert_eq!(valori, [H, 0.0, 2.0 * H, 0.0, 0.0, 0.0, 0.0]);
        let futuro: Vec<bool> = r.bars.iter().map(|b| b.future).collect();
        assert_eq!(futuro, [false, false, false, true, true, true, true]);
        assert_eq!(r.bars[0].tooltip, "lun 9: 1 h");
        assert_eq!(r.bars[3].tooltip, "gio 12: 0 min");
        assert_eq!(r.avg_workday_ms, Some(H));
        assert!(r.calls.is_empty());
    }

    #[test]
    fn media_settimana_passata() {
        let segs = vec![seg(ms(2026, 11, 7, 9, 0), ms(2026, 11, 7, 10, 0))];
        let r = report(
            &segs,
            Period::Week(date(2026, 11, 2)),
            date(2026, 11, 11),
            &tz(),
        );
        assert_eq!(r.avg_workday_ms, Some(H / 5.0));
        assert!(r.bars.iter().all(|b| !b.future));
    }

    #[test]
    fn report_mese_e_anno() {
        let segs = vec![seg(ms(2026, 10, 31, 23, 0), ms(2026, 11, 1, 1, 0))];
        let oggi = date(2026, 11, 11);
        let m = report(&segs, Period::Month(2026, 11), oggi, &tz());
        assert_eq!(m.bars.len(), 30);
        assert_eq!(m.bars[0].label, "1");
        assert_eq!(m.bars[29].label, "30");
        assert_eq!(m.bars[0].ms, H);
        assert_eq!(m.bars[12].tooltip, "13 nov: 0 min");
        assert!(!m.bars[10].future && m.bars[11].future);
        assert_eq!(m.total_ms, H);
        let a = report(&segs, Period::Year(2026), oggi, &tz());
        let etichette: String = a.bars.iter().map(|b| b.label.as_str()).collect();
        assert_eq!(etichette, "GFMAMGLASOND");
        assert_eq!(a.bars.len(), 12);
        assert_eq!(a.bars[9].ms, H);
        assert_eq!(a.bars[10].ms, H);
        assert_eq!(a.bars[10].tooltip, "novembre: 1 h");
        assert!(!a.bars[10].future && a.bars[11].future);
    }

    #[test]
    fn report_giorno() {
        let segs = vec![
            seg_app(ms(2026, 11, 11, 9, 0), ms(2026, 11, 11, 10, 0), "Teams"),
            seg_app(ms(2026, 11, 11, 14, 0), ms(2026, 11, 11, 14, 30), "Slack"),
        ];
        let r = report(
            &segs,
            Period::Day(date(2026, 11, 11)),
            date(2026, 11, 11),
            &tz(),
        );
        assert_eq!(r.calls.len(), 2);
        assert_eq!(r.calls[0].app, "Slack");
        assert_eq!(r.calls[1].app, "Teams");
        assert!(r.bars.is_empty());
        assert_eq!(r.avg_workday_ms, None);
        assert_eq!(
            r.per_app,
            vec![("Teams".to_string(), H), ("Slack".to_string(), 30.0 * MIN)]
        );
    }

    #[test]
    fn report_vuoto() {
        let r = report(
            &[],
            Period::Week(date(2026, 11, 9)),
            date(2026, 11, 11),
            &tz(),
        );
        assert_eq!(r.total_ms, 0.0);
        assert!(r.per_app.is_empty());
        assert_eq!(r.bars.len(), 7);
        assert!(r.bars.iter().all(|b| b.ms == 0.0));
    }

    #[test]
    fn taglio_intervallo() {
        let a = seg(ms(2026, 10, 5, 9, 0), ms(2026, 10, 5, 10, 0));
        let b = seg(ms(2026, 10, 31, 23, 0), ms(2026, 11, 1, 1, 0));
        let c = seg(ms(2026, 11, 3, 9, 0), ms(2026, 11, 3, 10, 0));
        let cut = cut_range(
            &[a.clone(), b, c.clone()],
            date(2026, 11, 1),
            date(2026, 11, 2),
            &tz(),
        );
        assert_eq!(cut.calls, 1);
        assert_eq!(cut.removed_ms, H);
        assert_eq!(
            cut.kept,
            vec![a, seg(ms(2026, 10, 31, 23, 0), ms(2026, 11, 1, 0, 0)), c]
        );
    }

    #[test]
    fn taglio_in_mezzo_divide() {
        let s = seg(ms(2026, 11, 1, 22, 0), ms(2026, 11, 3, 2, 0));
        let cut = cut_range(&[s], date(2026, 11, 2), date(2026, 11, 2), &tz());
        assert_eq!(cut.calls, 1);
        assert_eq!(cut.kept.len(), 2);
        assert_eq!(cut.kept[0].end_ms, ms(2026, 11, 2, 0, 0));
        assert_eq!(cut.kept[1].start_ms, ms(2026, 11, 3, 0, 0));
        assert_eq!(cut.removed_ms, 24.0 * H);
    }

    #[test]
    fn testo_conferma() {
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2026, 10, 1), date(2026, 10, 31)),
            "Cancellare 37 call (21 h 40 min) dal 1° al 31 ottobre 2026?"
        );
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2026, 9, 28), date(2026, 10, 3)),
            "Cancellare 37 call (21 h 40 min) dal 28 settembre al 3 ottobre 2026?"
        );
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2025, 12, 30), date(2026, 1, 2)),
            "Cancellare 37 call (21 h 40 min) dal 30 dicembre 2025 al 2 gennaio 2026?"
        );
        // elisione davanti a 8 e 11
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2026, 11, 1), date(2026, 11, 11)),
            "Cancellare 37 call (21 h 40 min) dal 1° all'11 novembre 2026?"
        );
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2026, 11, 8), date(2026, 11, 20)),
            "Cancellare 37 call (21 h 40 min) dall'8 al 20 novembre 2026?"
        );
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2026, 10, 11), date(2026, 11, 3)),
            "Cancellare 37 call (21 h 40 min) dall'11 ottobre al 3 novembre 2026?"
        );
        assert_eq!(
            confirm_text(37, 78_000_000.0, date(2026, 10, 2), date(2026, 10, 8)),
            "Cancellare 37 call (21 h 40 min) dal 2 all'8 ottobre 2026?"
        );
    }
}
