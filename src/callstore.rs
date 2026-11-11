//! Archivio su disco delle call dell'utente: un array JSON di tratti in `call.json`.
//! I dati restano sul PC. Ogni modifica viene salvata subito, in modo atomico.

use crate::callstats::{cut_range, Segment};
use chrono::{NaiveDate, TimeZone};
use std::path::PathBuf;

/// Un buco più lungo (sospensione del PC) non conta come tempo di call.
pub const GAP_MS: f64 = 60_000.0;

pub struct CallStore {
    path: PathBuf,
    segments: Vec<Segment>,
    /// Indice del tratto in corso (sempre l'ultimo).
    open: Option<usize>,
    /// Il file esistente non si può leggere né mettere da parte: non si scrive,
    /// per non perdere lo storico.
    sola_lettura: bool,
}

/// Rinomina il file illeggibile in un nome libero (`.danneggiato`, `.danneggiato.1`, ...).
/// Restituisce false se non ci riesce.
fn metti_da_parte(path: &PathBuf) -> bool {
    for n in 0..1000 {
        let mut nome = path.as_os_str().to_owned();
        nome.push(".danneggiato");
        if n > 0 {
            nome.push(format!(".{n}"));
        }
        let dest = PathBuf::from(nome);
        if dest.exists() {
            continue;
        }
        return match std::fs::rename(path, &dest) {
            Ok(()) => {
                crate::config::log("call.json danneggiato, messo da parte");
                true
            }
            Err(e) => {
                crate::config::log(format!(
                    "call.json illeggibile e non spostabile ({:?}): archivio in sola lettura",
                    e.kind()
                ));
                false
            }
        };
    }
    crate::config::log("call.json illeggibile: nessun nome libero, archivio in sola lettura");
    false
}

fn valido(s: &Segment) -> bool {
    s.start_ms.is_finite() && s.end_ms.is_finite() && s.end_ms > s.start_ms
}

impl CallStore {
    /// File mancante = vuoto. File illeggibile = messo da parte come `<nome>.danneggiato`.
    /// Non riapre mai un tratto: quello rimasto dal giro precedente finisce al suo `end_ms`.
    pub fn load(path: PathBuf) -> CallStore {
        let mut segments = Vec::new();
        let mut sola_lettura = false;
        match std::fs::read_to_string(&path) {
            Ok(testo) => match serde_json::from_str::<Vec<Segment>>(&testo) {
                Ok(v) => segments = v.into_iter().filter(valido).collect(),
                Err(_) => sola_lettura = !metti_da_parte(&path),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => sola_lettura = !metti_da_parte(&path),
        }
        CallStore {
            path,
            segments,
            open: None,
            sola_lettura,
        }
    }

    #[cfg(test)]
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Copia dei tratti con quello aperto esteso fino a `now_ms`.
    pub fn snapshot(&self, now_ms: f64) -> Vec<Segment> {
        let mut v = self.segments.clone();
        if let Some(i) = self.open {
            // Un buco oltre GAP_MS (sospensione) non conta come tempo di call. Un tratto
            // appena aperto, mai aggiornato, si estende comunque fino a `now`.
            let s = &v[i];
            let appena_aperto = s.end_ms == s.start_ms;
            if now_ms.is_finite()
                && now_ms > s.end_ms
                && (appena_aperto || now_ms - s.end_ms <= GAP_MS)
            {
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
        self.segments = cut.kept;
        self.open = None;
        if let Some(s) = aperto {
            if toccato {
                self.apri(&s.app, s.auto, now_ms);
            } else if self
                .segments
                .last()
                .is_some_and(|u| u.start_ms == s.start_ms)
            {
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

    /// Scrittura atomica: file `.tmp` e poi `rename`. Gli errori si registrano nel log
    /// (mai i dati delle call).
    fn save(&self) {
        if self.sola_lettura {
            return;
        }
        let json = match serde_json::to_string(&self.segments) {
            Ok(j) => j,
            Err(_) => {
                crate::config::log("call.json: serializzazione fallita");
                return;
            }
        };
        if let Some(dir) = self.path.parent() {
            if let Err(e) = std::fs::create_dir_all(dir) {
                crate::config::log(format!("call.json: cartella non creata ({:?})", e.kind()));
                return;
            }
        }
        let mut tmp = self.path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        if let Err(e) = std::fs::write(&tmp, json) {
            crate::config::log(format!("call.json: scrittura fallita ({:?})", e.kind()));
            let _ = std::fs::remove_file(&tmp);
            return;
        }
        if let Err(e) = std::fs::rename(&tmp, &self.path) {
            crate::config::log(format!("call.json: rename fallito ({:?})", e.kind()));
            let _ = std::fs::remove_file(&tmp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};

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

    #[test]
    fn file_non_utf8_messo_da_parte() {
        let d = dir("nonutf8");
        let p = d.join("call.json");
        std::fs::write(&p, [0xff, 0xfe, 0x00, 0x9f]).unwrap();
        let mut s = CallStore::load(p.clone());
        assert!(s.segments().is_empty());
        let bak = d.join("call.json.danneggiato");
        assert_eq!(std::fs::read(&bak).unwrap(), vec![0xff, 0xfe, 0x00, 0x9f]);
        s.begin(Some("Teams"), true, T);
        s.end(T + 5.0 * S);
        assert_eq!(std::fs::read(&bak).unwrap(), vec![0xff, 0xfe, 0x00, 0x9f]);
    }

    #[test]
    fn seconda_corruzione_non_distrugge_la_prima() {
        let d = dir("doppio");
        let p = d.join("call.json");
        std::fs::write(d.join("call.json.danneggiato"), "prima").unwrap();
        std::fs::write(&p, "{seconda").unwrap();
        let _ = CallStore::load(p);
        assert_eq!(
            std::fs::read_to_string(d.join("call.json.danneggiato")).unwrap(),
            "prima"
        );
        assert_eq!(
            std::fs::read_to_string(d.join("call.json.danneggiato.1")).unwrap(),
            "{seconda"
        );
    }

    #[test]
    fn file_non_spostabile_non_viene_sovrascritto() {
        // Simula il caso in cui il file non si può mettere da parte: non si scrive nulla.
        let d = dir("sololettura");
        let p = d.join("call.json");
        std::fs::write(&p, "{rotto").unwrap();
        let mut s = CallStore::load(p.clone());
        s.sola_lettura = true;
        s.begin(Some("Teams"), true, T);
        assert!(!p.exists());
    }

    #[test]
    fn cancella_dopo_sospensione_non_conta_il_buco() {
        let tz = Utc;
        let oggi = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let ieri = oggi - Duration::days(1);
        let t0 = crate::callstats::day_start_ms(oggi, &tz) + 9.0 * 3600.0 * S;
        let mut s = CallStore::load(dir("buco").join("call.json"));
        s.begin(Some("Teams"), true, t0);
        s.touch(t0 + 20.0 * S);
        let now = t0 + 20.0 * S + 2.0 * 3600.0 * S;
        assert_eq!(s.snapshot(now)[0].end_ms, t0 + 20.0 * S);
        s.delete_range(ieri, ieri, now, &tz);
        assert_eq!(s.segments().len(), 1);
        assert_eq!(s.segments()[0].end_ms, t0 + 20.0 * S);
    }

    #[test]
    fn tratto_aperto_di_durata_zero_resta_aperto_dopo_cancellazione() {
        let tz = Utc;
        let oggi = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        let altro = oggi - Duration::days(5);
        let ieri = oggi - Duration::days(1);
        let ms = |d: NaiveDate, h: f64| crate::callstats::day_start_ms(d, &tz) + h * 3600.0 * S;
        let mut s = CallStore::load(dir("zero").join("call.json"));
        s.begin(Some("Meet"), false, ms(ieri, 10.0));
        s.end(ms(ieri, 10.0) + 60.0 * S);
        s.begin(Some("Teams"), true, ms(oggi, 9.0));
        s.delete_range(altro, altro, ms(oggi, 9.0), &tz);
        assert!(s.is_open());
        s.touch(ms(oggi, 9.0) + 20.0 * S);
        assert_eq!(s.segments().len(), 2);
        assert_eq!(s.segments()[0].end_ms - s.segments()[0].start_ms, 60.0 * S);
        assert_eq!(s.segments()[1].end_ms - s.segments()[1].start_ms, 20.0 * S);
    }
}
