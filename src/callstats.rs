//! Statistiche delle call: logica pura sulle date (periodi, divisione a mezzanotte, formati).
//! Fuso locale, settimana lunedì–domenica, giorni interi, nomi italiani senza locale di sistema.

// Le funzioni vengono usate dai moduli successivi: l'attributo si toglie quando il modulo
// è collegato al resto dell'app (Task 5).
#![allow(dead_code)]

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
}
