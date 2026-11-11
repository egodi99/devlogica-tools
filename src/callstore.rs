//! Archivio su disco delle call dell'utente: un array JSON di tratti in `call.json`.
//! I dati restano sul PC. Ogni modifica viene salvata subito, in modo atomico.

// Va tolto quando l'archivio sarà collegato all'app (Task 4/5).
#![allow(dead_code)]

use crate::callstats::{cut_range, Segment};
use chrono::{Duration, NaiveDate, TimeZone};
use std::path::PathBuf;

/// Un buco più lungo (sospensione del PC) non conta come tempo di call.
pub const GAP_MS: f64 = 60_000.0;

pub struct CallStore {
    path: PathBuf,
    segments: Vec<Segment>,
    /// Indice del tratto in corso (sempre l'ultimo).
    open: Option<usize>,
}

fn valido(s: &Segment) -> bool {
    s.start_ms.is_finite() && s.end_ms.is_finite() && s.end_ms > s.start_ms
}

impl CallStore {
    /// File mancante = vuoto. File illeggibile = messo da parte come `<nome>.danneggiato`.
    /// Non riapre mai un tratto: quello rimasto dal giro precedente finisce al suo `end_ms`.
    pub fn load(path: PathBuf) -> CallStore {
        let mut segments = Vec::new();
        if let Ok(testo) = std::fs::read_to_string(&path) {
            match serde_json::from_str::<Vec<Segment>>(&testo) {
                Ok(v) => segments = v.into_iter().filter(valido).collect(),
                Err(_) => {
                    let mut nome = path.as_os_str().to_owned();
                    nome.push(".danneggiato");
                    let _ = std::fs::rename(&path, PathBuf::from(nome));
                    crate::config::log("call.json danneggiato, messo da parte");
                }
            }
        }
        CallStore {
            path,
            segments,
            open: None,
        }
    }

    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Copia dei tratti con quello aperto esteso fino a `now_ms`.
    pub fn snapshot(&self, now_ms: f64) -> Vec<Segment> {
        let mut v = self.segments.clone();
        if let Some(i) = self.open {
            if now_ms.is_finite() && now_ms > v[i].end_ms {
                v[i].end_ms = now_ms;
            }
        }
        v
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn apri(&mut self, app: &str, auto: bool, now_ms: f64) {
        self.segments.push(Segment {
            start_ms: now_ms,
            end_ms: now_ms,
            app: app.to_string(),
            auto,
        });
        self.open = Some(self.segments.len() - 1);
    }

    /// Inizia un tratto; senza app vale "Manuale". Se è già aperto non fa niente.
    pub fn begin(&mut self, app: Option<&str>, auto: bool, now_ms: f64) {
        if self.open.is_some() || !now_ms.is_finite() {
            return;
        }
        self.apri(app.unwrap_or("Manuale"), auto, now_ms);
        self.save();
    }

    /// Aggiorna la fine. Con un buco oltre `GAP_MS` chiude all'ultimo salvataggio e apre
    /// un nuovo tratto da `now_ms`.
    pub fn touch(&mut self, now_ms: f64) {
        let Some(i) = self.open else { return };
        if !now_ms.is_finite() {
            return;
        }
        if now_ms - self.segments[i].end_ms > GAP_MS {
            let (app, auto) = (self.segments[i].app.clone(), self.segments[i].auto);
            self.apri(&app, auto, now_ms);
        } else if now_ms > self.segments[i].end_ms {
            self.segments[i].end_ms = now_ms;
        }
        self.save();
    }

    /// Come `touch`, ma senza riaprire: con un buco oltre `GAP_MS` il tratto resta
    /// com'era all'ultimo salvataggio.
    pub fn end(&mut self, now_ms: f64) {
        let Some(i) = self.open.take() else { return };
        if now_ms.is_finite()
            && now_ms - self.segments[i].end_ms <= GAP_MS
            && now_ms > self.segments[i].end_ms
        {
            self.segments[i].end_ms = now_ms;
        }
        self.save();
    }

    /// Quante call e quanto tempo toglierebbe `delete_range`.
    pub fn preview_delete<Tz: TimeZone>(
        &self,
        from: NaiveDate,
        to: NaiveDate,
        now_ms: f64,
        tz: &Tz,
    ) -> (usize, f64) {
        let c = cut_range(&self.snapshot(now_ms), from, to, tz);
        (c.calls, c.removed_ms)
    }

    /// Toglie dallo storico i giorni da `from` a `to` compresi. Se il tratto aperto viene
    /// toccato, ne apre uno nuovo da `now_ms` con la stessa app.
    pub fn delete_range<Tz: TimeZone>(
        &mut self,
        from: NaiveDate,
        to: NaiveDate,
        now_ms: f64,
        tz: &Tz,
    ) {
        let snap = self.snapshot(now_ms);
        let aperto = self.open.map(|i| snap[i].clone());
        let toccato = match &aperto {
            Some(s) => cut_range(std::slice::from_ref(s), from, to, tz).calls > 0,
            None => false,
        };
        let cut = cut_range(&snap, from, to, tz);
        self.segments = cut.kept.into_iter().filter(valido).collect();
        self.open = None;
        if let Some(s) = aperto {
            if toccato {
                self.apri(&s.app, s.auto, now_ms);
            } else if !self.segments.is_empty() {
                // Il tratto aperto non è stato toccato: è rimasto l'ultimo.
                self.open = Some(self.segments.len() - 1);
            }
        }
        self.save();
    }

    /// Primo giorno con una call, per il pulsante "Tutto".
    pub fn first_day<Tz: TimeZone>(&self, tz: &Tz) -> Option<NaiveDate> {
        self.segments
            .iter()
            .map(|s| s.start_ms)
            .min_by(|a, b| a.total_cmp(b))
            .map(|ms| crate::callstats::local_date(ms, tz))
    }

    /// Scrittura atomica: file `.tmp` e poi `rename`.
    fn save(&self) {
        let Ok(json) = serde_json::to_string(&self.segments) else {
            return;
        };
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        if std::fs::write(&tmp, json).is_ok() {
            let _ = std::fs::rename(&tmp, &self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dlt-callstore-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const T: f64 = 1_700_000_000_000.0;
    const S: f64 = 1000.0;

    #[test]
    fn registra_e_rilegge() {
        let p = dir("rilegge").join("call.json");
        let mut s = CallStore::load(p.clone());
        s.begin(Some("Teams"), true, T);
        s.touch(T + 20.0 * S);
        s.end(T + 40.0 * S);
        let r = CallStore::load(p);
        assert_eq!(r.segments().len(), 1);
        assert_eq!(r.segments()[0].end_ms - r.segments()[0].start_ms, 40.0 * S);
        assert_eq!(r.segments()[0].app, "Teams");
        assert!(r.segments()[0].auto);
    }

    #[test]
    fn call_manuale_senza_app() {
        let mut s = CallStore::load(dir("manuale").join("call.json"));
        s.begin(None, false, T);
        assert_eq!(s.snapshot(T)[0].app, "Manuale");
    }

    #[test]
    fn sospensione_non_conta() {
        let mut s = CallStore::load(dir("sosp").join("call.json"));
        let due_ore = 2.0 * 3600.0 * S;
        s.begin(Some("Zoom"), true, T);
        s.touch(T + 20.0 * S);
        s.touch(T + 20.0 * S + due_ore);
        assert_eq!(s.segments().len(), 2);
        assert_eq!(s.segments()[0].end_ms, T + 20.0 * S);
        assert_eq!(s.segments()[1].start_ms, T + 20.0 * S + due_ore);
        assert!(s.is_open());
    }

    #[test]
    fn fine_dopo_sospensione_non_crea_tratti() {
        let mut s = CallStore::load(dir("finesosp").join("call.json"));
        s.begin(Some("Zoom"), true, T);
        s.touch(T + 20.0 * S);
        s.end(T + 20.0 * S + 8.0 * 3600.0 * S);
        assert_eq!(s.segments().len(), 1);
        assert_eq!(s.segments()[0].end_ms, T + 20.0 * S);
        assert!(!s.is_open());
    }

    #[test]
    fn riavvio_chiude_il_tratto_aperto() {
        let p = dir("riavvio").join("call.json");
        let mut s = CallStore::load(p.clone());
        s.begin(Some("Teams"), true, T);
        s.touch(T + 20.0 * S);
        let mut r = CallStore::load(p);
        assert!(!r.is_open());
        r.begin(Some("Teams"), true, T + 3600.0 * S);
        assert_eq!(r.segments().len(), 2);
        assert_eq!(r.segments()[0].end_ms, T + 20.0 * S);
    }

    #[test]
    fn file_danneggiato_messo_da_parte() {
        let d = dir("rotto");
        let p = d.join("call.json");
        std::fs::write(&p, "{rotto").unwrap();
        let mut s = CallStore::load(p.clone());
        assert!(s.segments().is_empty());
        let bak = d.join("call.json.danneggiato");
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), "{rotto");
        s.begin(Some("Teams"), true, T);
        s.end(T + 5.0 * S);
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), "{rotto");
        let v: Vec<Segment> = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v.len(), 1);
    }

    #[test]
    fn tratti_non_validi_scartati() {
        let d = dir("invalidi");
        let p = d.join("call.json");
        let json = r#"[
            {"start_ms":100.0,"end_ms":100.0,"app":"A","auto":true},
            {"start_ms":200.0,"end_ms":100.0,"app":"B","auto":true},
            {"start_ms":null,"end_ms":100.0,"app":"C","auto":true},
            {"start_ms":1000.0,"end_ms":2000.0,"app":"D","auto":true}
        ]"#;
        std::fs::write(&p, json).unwrap();
        let s = CallStore::load(p);
        // null non è un numero valido per f64: il file intero può risultare danneggiato,
        // ma in ogni caso nessun tratto non valido deve restare.
        assert!(s.segments().iter().all(valido));
        let p2 = d.join("b.json");
        std::fs::write(
            &p2,
            r#"[{"start_ms":100.0,"end_ms":100.0,"app":"A","auto":true},
                {"start_ms":200.0,"end_ms":100.0,"app":"B","auto":true},
                {"start_ms":1000.0,"end_ms":2000.0,"app":"D","auto":true}]"#,
        )
        .unwrap();
        let s2 = CallStore::load(p2);
        assert_eq!(s2.segments().len(), 1);
        assert_eq!(s2.segments()[0].app, "D");
    }

    #[test]
    fn snapshot_estende_la_call_in_corso() {
        let mut s = CallStore::load(dir("snap").join("call.json"));
        s.begin(Some("Teams"), true, T);
        let snap = s.snapshot(T + 300.0 * S);
        assert_eq!(snap[0].end_ms - snap[0].start_ms, 300.0 * S);
        assert_eq!(s.segments()[0].end_ms - s.segments()[0].start_ms, 0.0);
    }

    #[test]
    fn cancella_intervallo_con_call_in_corso() {
        let tz = Utc;
        let oggi = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let ieri = oggi - Duration::days(1);
        let ms = |d: NaiveDate, h: f64| crate::callstats::day_start_ms(d, &tz) + h * 3600.0 * S;
        let p = dir("cancella").join("call.json");
        let mut s = CallStore::load(p.clone());
        s.begin(Some("Meet"), false, ms(ieri, 10.0));
        s.touch(ms(ieri, 10.0) + 30.0 * S);
        s.end(ms(ieri, 10.0) + 60.0 * S);
        s.begin(Some("Teams"), true, ms(oggi, 9.0));
        let now = ms(oggi, 11.0);
        assert_eq!(
            s.preview_delete(oggi, oggi, now, &tz),
            (1, 2.0 * 3600.0 * S)
        );
        s.delete_range(oggi, oggi, now, &tz);
        assert_eq!(s.segments().len(), 2);
        assert_eq!(s.segments()[0].app, "Meet");
        assert_eq!(s.segments()[1].start_ms, now);
        assert_eq!(s.segments()[1].app, "Teams");
        assert!(s.segments()[1].auto);
        assert!(s.is_open());
        // Un tratto appena aperto è lungo zero e su disco vale solo dopo il primo touch.
        s.touch(now + 20.0 * S);
        assert_eq!(CallStore::load(p).segments().len(), 2);
        assert_eq!(s.first_day(&tz), Some(ieri));
    }
}
