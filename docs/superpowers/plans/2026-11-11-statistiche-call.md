# Statistiche delle ore di call Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** registrare le call dell'utente su disco e mostrarle nella finestra "Le mie call" con le schede Giorno, Settimana, Mese e Anno, più l'interruttore della registrazione e la cancellazione per intervallo di date nelle Impostazioni.

**Architecture:**
- **`src/callstats.rs`**: logica pura e testata. Contiene periodi, divisione a mezzanotte, report, medie, formati, taglio di un intervallo. È generica sul fuso (`chrono::TimeZone`): i test usano `FixedOffset`, l'app `chrono::Local`.
- **`src/callstore.rs`**: archivio su `call.json`. Registra il tratto in corso, gestisce i buchi di sospensione, salva in modo atomico e mette da parte i file danneggiati. È testato su cartelle temporanee.
- **`src/statsui.rs`**: la finestra egui, che si limita a disegnare un `Report` già calcolato.
- **Collegamenti**: `app.rs` chiama l'archivio a inizio call, a ogni CALL_ALIVE, a fine call e alla chiusura dell'app. Il resto si aggancia a menu, Impostazioni e anteprima.

**Tech Stack:** Rust 1.95+, egui 0.36, `chrono` (ora locale), `egui_extras` 0.36 con la feature `datepicker` per scegliere le date.

**Spec:** `docs/superpowers/specs/2026-11-11-statistiche-call-design.md` (bozza approvata: opzione A in `.superpowers/brainstorm/19658-1794388580/content/statistiche-layout.html`)

## Global Constraints

- I dati non escono mai dal PC: niente nei pacchetti UDP e nessuna call nel log.
- File: `config::app_dir().join("call.json")`. Se non si legge va rinominato in `call.json.danneggiato` e non si sovrascrive mai. La scrittura è atomica: `call.json.tmp` e poi rinomina.
- Fuso orario locale, settimana da lunedì a domenica, giorni interi. Una call a cavallo della mezzanotte si divide sui due giorni.
- Durate: `"2 h 10 min"`, `"2 h"` (minuti a zero), `"45 min"`, `"< 1 min"` (meno di 60 s, ma più di 0). Zero vale `"0 min"`.
- App sconosciuta o call manuale senza app: `"Manuale"`.
- Buco di sospensione: `GAP_MS = 60_000.0`. Un buco più lungo tra due salvataggi non conta.
- Salvataggio della call in corso: all'inizio, a ogni CALL_ALIVE (20 s, `next_alive` in `app.rs`) e alla fine.
- Media per giorno lavorativo = totale ÷ giorni lun–ven del periodo fino a oggi compreso. Non c'è nella scheda Giorno. Con zero giorni lavorativi non si mostra.
- La scheda attiva all'apertura è Settimana, sulla settimana corrente. La freccia › non va oltre il periodo in corso.
- `Config::record_calls: bool`, default `true`.
- Testi esatti:
  - menu: `Le mie call…`;
  - finestra: titolo `DevLogica Tools – Le mie call`, intestazione `Le mie call`, schede `Giorno` `Settimana` `Mese` `Anno`, sezione `Per app`;
  - nessuna call: `Nessuna call`; registrazione spenta: `Registrazione disattivata`;
  - Impostazioni: sezione `Le mie call`, pulsante `Le mie call…`, interruttore `Registra le mie call`, `Cancella lo storico…`, campi `Dal` e `Al`, pulsante `Tutto`;
  - conferma: `Cancellare {n} call ({durata}) dal {da} al {a}?`, con i pulsanti `Cancella` e `Annulla`.
- Lo stile è quello di `ui.rs`: costanti `INK`, `TEXT`, `MUTED`, `RED`, e `fn section`. Testi e commenti in italiano.

## Review Focus

1. **Portatile chiuso durante una call riconosciuta in automatico.** Al risveglio il rilevatore chiude la call dopo circa 10 s: le ore di sonno non contano e non nasce un tratto vuoto di 0 secondi. Test `fine_dopo_sospensione_non_crea_tratti` (Task 3).
2. **App chiusa di colpo durante una call e poi riaperta.** Il tratto rimasto nel file finisce all'ultimo salvataggio e non viene riaperto, e una nuova call crea un tratto nuovo. Test `riavvio_chiude_il_tratto_aperto` (Task 3).
3. **File danneggiato.** Viene messo da parte e non sovrascritto. Il salvataggio successivo crea un `call.json` nuovo e il `.danneggiato` resta intatto. Test `file_danneggiato_messo_da_parte` (Task 3).
4. **Settimana a cavallo di mese e di anno.** Etichetta e barre corrette per 29 dicembre 2025 – 4 gennaio 2026. Test `settimana_a_cavallo_dell_anno` (Task 1).
5. **Cancellare un intervallo che comprende la call in corso.** La parte registrata sparisce e la registrazione continua da adesso. Test `cancella_intervallo_con_call_in_corso` (Task 3).

---

### Task 1: periodi, divisione a mezzanotte, formati (`callstats.rs`)

**Files:**
- Create: `src/callstats.rs`
- Modify: `src/main.rs` (aggiungere `mod callstats;`), `Cargo.toml` (aggiungere `chrono`, default-features off, features `clock` e `std`)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
  pub struct Segment { pub start_ms: f64, pub end_ms: f64, pub app: String, pub auto: bool }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum PeriodKind { Day, Week, Month, Year }

  #[derive(Clone, Copy, Debug, PartialEq, Eq)]
  pub enum Period { Day(NaiveDate), Week(NaiveDate /* lunedì */), Month(i32, u32), Year(i32) }
  impl Period {
      pub fn containing(kind: PeriodKind, d: NaiveDate) -> Period;
      pub fn kind(&self) -> PeriodKind;
      pub fn first_day(&self) -> NaiveDate;
      pub fn last_day(&self) -> NaiveDate;          // compreso
      pub fn prev(&self) -> Period;
      pub fn next(&self) -> Period;
      pub fn can_go_next(&self, today: NaiveDate) -> bool; // false se next() inizia dopo oggi
      pub fn label(&self, today: NaiveDate) -> String;
  }
  pub fn per_day<Tz: TimeZone>(seg: &Segment, tz: &Tz) -> Vec<(NaiveDate, f64)>; // ms per giorno locale
  pub fn workdays(from: NaiveDate, to: NaiveDate) -> u32;  // lun–ven, estremi compresi
  pub fn fmt_duration(ms: f64) -> String;
  pub fn local_date<Tz: TimeZone>(ms: f64, tz: &Tz) -> NaiveDate;
  pub fn day_start_ms<Tz: TimeZone>(d: NaiveDate, tz: &Tz) -> f64; // mezzanotte locale in ms
  ```
- I confini tra i giorni si calcolano in ms reali con `day_start_ms`, non con l'ora dell'orologio. Così un cambio dell'ora legale durante una call non aggiunge e non toglie un'ora.

- [ ] **Step 1: scrivi i test** in `src/callstats.rs` (`#[cfg(test)]`), con `tz = FixedOffset::east_opt(3600)` e un helper `ms(y,m,d,h,min)` che restituisce i ms di quell'ora locale:
  - `formati_durata`: `fmt_duration(0.0)=="0 min"`, `(30_000.0)=="< 1 min"`, `(45*60_000)=="45 min"`, `(2h10m)=="2 h 10 min"`, `(2h)=="2 h"`.
  - `divide_a_mezzanotte`: una call dalle 23:30 del 10/11/2026 all'01:15 dell'11/11 dà `[(10/11, 30 min), (11/11, 75 min)]`.
  - `call_nello_stesso_giorno`: un solo elemento.
  - `periodi_contenenti`: `containing(Week, mer 11/11/2026)` == `Week(lun 9/11)`, `last_day` == dom 15/11. `containing(Month, 11/11)` == `Month(2026,11)` con `last_day` 30/11. Per febbraio 2028 `last_day` è il 29/2.
  - `navigazione`: `Week(9/11).prev()==Week(2/11)`, `Month(2026,1).prev()==Month(2025,12)`, `Year(2026).next()==Year(2027)`. `can_go_next(today=11/11/2026)` è falso per `Week(9/11)` e vero per `Week(2/11)`.
  - `etichette`: con `today` = 11/11/2026, `Day(11/11)` → `"oggi"`, `Day(10/11)` → `"ieri"`, `Day(3/11)` → `"martedì 3 novembre 2026"`, `Week(9/11)` → `"9–15 novembre"`, `Month(2026,11)` → `"novembre 2026"`, `Year(2026)` → `"2026"`.
  - `settimana_a_cavallo_dell_anno`: `Week(29/12/2025).label(...)` == `"29 dicembre 2025 – 4 gennaio 2026"`. Per una settimana a cavallo di due mesi nello stesso anno: `"27 ottobre – 2 novembre"`.
  - `giorni_lavorativi`: `workdays(9/11, 15/11)==5`, `workdays(9/11, 11/11)==3`, `workdays(14/11, 15/11)==0`.

- [ ] **Step 2: esegui** `cargo test callstats`. Atteso: errori di compilazione (le funzioni non esistono).

- [ ] **Step 3: implementa** le interfacce qui sopra. I nomi italiani di mesi e giorni stanno in costanti locali al modulo, senza locale di sistema. `per_day` scorre i giorni da `local_date(start)` a `local_date(end)` e taglia ogni pezzo con `day_start_ms`.

- [ ] **Step 4: esegui** `cargo test callstats`. Atteso: PASS, e tutto `cargo test` resta verde.

- [ ] **Step 5: commit** `feat(stats): periodi, divisione per giorno e formati`

### Task 2: report e taglio di un intervallo (`callstats.rs`)

**Files:**
- Modify: `src/callstats.rs`

**Interfaces:**
- Consumes: Task 1.
- Produces:
  ```rust
  pub struct Bar { pub label: String, pub tooltip: String, pub ms: f64, pub future: bool }
  pub struct CallRow { pub start_ms: f64, pub end_ms: f64, pub app: String } // per la scheda Giorno
  pub struct Report {
      pub total_ms: f64,
      pub avg_workday_ms: Option<f64>,   // None nella scheda Giorno o con 0 giorni lavorativi
      pub bars: Vec<Bar>,                // vuoto nella scheda Giorno
      pub calls: Vec<CallRow>,           // solo nella scheda Giorno, dalla più recente
      pub per_app: Vec<(String, f64)>,   // per durata decrescente
  }
  pub fn report<Tz: TimeZone>(segs: &[Segment], p: Period, today: NaiveDate, tz: &Tz) -> Report;
  pub struct Cut { pub kept: Vec<Segment>, pub calls: usize, pub removed_ms: f64 }
  pub fn cut_range<Tz: TimeZone>(segs: &[Segment], from: NaiveDate, to: NaiveDate, tz: &Tz) -> Cut;
  ```
- Barre:
  - Settimana: 7 barre `lun`…`dom`, tooltip `"gio 13: 3 h 20 min"`;
  - Mese: una barra per giorno con etichetta `"1"`…`"30"`, tooltip `"13 nov: 3 h 20 min"`;
  - Anno: 12 barre `G F M A M G L A S O N D`, tooltip `"novembre: 27 h"`.
- `future = true` per i giorni (o i mesi) successivi a `today`.
- Nella scheda Giorno, i `CallRow` sono la parte del tratto che cade in quel giorno.
- `cut_range` toglie dai tratti la parte tra `day_start_ms(from)` e `day_start_ms(to + 1 giorno)`. Un tratto può restare intero, accorciarsi o dividersi in due. `calls` conta i tratti toccati.

- [ ] **Step 1: scrivi i test**:
  - `report_settimana`: tratti di 1 h lunedì 9/11, 2 h mercoledì 11/11 e 30 min domenica 8/11 (settimana prima, quindi fuori), con today = 11/11. Atteso: `total_ms` = 3 h, 7 barre con i valori giusti, `future` vero da giovedì a domenica, `avg_workday_ms` = 3 h ÷ 3 (lun–mer fino a oggi).
  - `media_settimana_passata`: per `Week(2/11)` con 1 h sabato 7/11 e niente altro: `avg_workday_ms` = 1 h ÷ 5, `future` sempre falso.
  - `report_mese_e_anno`: novembre 2026 ha 30 barre e l'anno ha 12 barre con le etichette `G`…`D`. Le ore di un tratto a cavallo di mezzanotte del 31/10 vanno metà nella barra di ottobre e metà in quella di novembre.
  - `report_giorno`: due call (Teams 9:00–10:00 e Slack 14:00–14:30). `calls` contiene Slack e poi Teams, `bars` è vuoto, `avg_workday_ms` è None, `per_app` è `[("Teams", 1 h), ("Slack", 30 min)]`.
  - `report_vuoto`: nessun tratto. `total_ms == 0`, `per_app` vuoto, le barre ci sono comunque a zero.
  - `taglio_intervallo`: tratti il 5/10, dal 31/10 23:00 all'1/11 01:00 e il 3/11. `cut_range(1/11, 2/11)` lascia intatti il 5/10 e il 3/11, accorcia quello a cavallo a 31/10 23:00–24:00, e dà `calls == 1`, `removed_ms` = 1 h.
  - `taglio_in_mezzo_divide`: un tratto dal 1/11 alle 22:00 al 3/11 alle 02:00 con `cut_range(2/11, 2/11)` diventa due tratti.

- [ ] **Step 2: esegui** `cargo test callstats`. Atteso: FAIL in compilazione.

- [ ] **Step 3: implementa** `report` e `cut_range`.

- [ ] **Step 4: esegui** `cargo test`. Atteso: PASS.

- [ ] **Step 5: commit** `feat(stats): report del periodo e taglio di un intervallo`

### Task 3: archivio su disco (`callstore.rs`)

**Files:**
- Create: `src/callstore.rs`
- Modify: `src/main.rs` (`mod callstore;`)

**Interfaces:**
- Consumes: `Segment`, `cut_range`, `Cut` (Task 1–2).
- Produces:
  ```rust
  pub const GAP_MS: f64 = 60_000.0;
  pub struct CallStore { /* path, segments, open: Option<usize> */ }
  impl CallStore {
      pub fn load(path: PathBuf) -> CallStore;            // file mancante = vuoto; illeggibile = rinomina in "<nome>.danneggiato"
      pub fn segments(&self) -> &[Segment];
      pub fn snapshot(&self, now_ms: f64) -> Vec<Segment>; // copia con il tratto aperto esteso fino a now
      pub fn is_open(&self) -> bool;
      pub fn begin(&mut self, app: Option<&str>, auto: bool, now_ms: f64); // None -> "Manuale"; se già aperto non fa niente
      pub fn touch(&mut self, now_ms: f64);   // aggiorna la fine; buco > GAP_MS: chiude all'ultimo salvataggio e apre un nuovo tratto da now
      pub fn end(&mut self, now_ms: f64);     // come touch ma senza riaprire: con un buco > GAP_MS chiude all'ultimo salvataggio
      pub fn preview_delete<Tz: TimeZone>(&self, from: NaiveDate, to: NaiveDate, now_ms: f64, tz: &Tz) -> (usize, f64);
      pub fn delete_range<Tz: TimeZone>(&mut self, from: NaiveDate, to: NaiveDate, now_ms: f64, tz: &Tz);
      pub fn first_day<Tz: TimeZone>(&self, tz: &Tz) -> Option<NaiveDate>; // per il pulsante "Tutto"
  }
  ```
- Ogni metodo che cambia i dati salva subito, in modo atomico (`.tmp` e poi `rename`). Il file contiene un array JSON di `Segment`.
- `load` non riapre mai un tratto: quello rimasto dal giro precedente finisce al suo `end_ms` salvato.
- `delete_range` usa `snapshot(now_ms)` e `cut_range`. Se il tratto aperto è stato toccato, ne apre uno nuovo da `now_ms` con la stessa app e lo stesso `auto`.

- [ ] **Step 1: scrivi i test** con una cartella temporanea per ogni test (come `dir()` in `install.rs`):
  - `registra_e_rilegge`: `begin(Some("Teams"), true, t)`, `touch(t+20s)`, `end(t+40s)`. `load` dello stesso file dà un tratto di 40 s.
  - `call_manuale_senza_app`: `begin(None, false, t)` registra l'app come `"Manuale"`.
  - `sospensione_non_conta`: `begin(t)`, `touch(t+20s)`, `touch(t+20s+2h)`. Risultato: due tratti, il primo fino a t+20s e il secondo che inizia a t+20s+2h.
  - `fine_dopo_sospensione_non_crea_tratti`: `begin(t)`, `touch(t+20s)`, `end(t+20s+8h)`. Risultato: un solo tratto fino a t+20s e `is_open()` falso.
  - `riavvio_chiude_il_tratto_aperto`: `begin(t)`, `touch(t+20s)`, poi `load` di nuovo senza `end`. `is_open()` è falso. Un nuovo `begin(t+1h)` crea un secondo tratto senza modificare il primo.
  - `file_danneggiato_messo_da_parte`: scrivi `"{rotto"` in `call.json` e poi `load`. Il risultato è vuoto, esiste `call.json.danneggiato` con `"{rotto"`, e dopo `begin`/`end` il `.danneggiato` è ancora intatto e `call.json` è valido.
  - `snapshot_estende_la_call_in_corso`: `begin(t)` e `snapshot(t+5min)` dà un tratto di 5 min, mentre `segments()` resta a 0.
  - `cancella_intervallo_con_call_in_corso`: un tratto chiuso ieri e una call aperta da oggi alle 9:00, con `now` alle 11:00. `delete_range(oggi, oggi, now)` lascia il tratto di ieri e un tratto aperto che parte da `now`. `preview_delete` prima della cancellazione dava `(1, 2 h)`.

- [ ] **Step 2: esegui** `cargo test callstore`. Atteso: FAIL in compilazione.

- [ ] **Step 3: implementa** `CallStore`.

- [ ] **Step 4: esegui** `cargo test`. Atteso: PASS.

- [ ] **Step 5: commit** `feat(stats): archivio delle call con salvataggio atomico`

### Task 4: registrazione dalle call reali e interruttore in configurazione

**Files:**
- Modify: `src/config.rs` (campo `record_calls`), `src/app.rs` (`start_call`, `end_call`, il blocco `next_alive` in `about_to_wait`, `shutdown`, `handle_ui` per `ConfigChanged`)

**Interfaces:**
- Consumes: `CallStore` (Task 3).
- Produces: `App.store: CallStore`, caricato in `App::new` da `config::app_dir().join("call.json")`. `Config::record_calls: bool`.

- [ ] **Step 1: scrivi il test** in `config.rs`: `record_calls_attivo_di_default`. Verifica che `Config::default().record_calls` sia vero e che `serde_json::from_str::<Config>("{}")` dia `record_calls == true`, così un `config.json` vecchio la attiva.

- [ ] **Step 2: esegui** `cargo test config`. Atteso: FAIL in compilazione.

- [ ] **Step 3: implementa**:
  - il campo `record_calls` in `Config` e nel suo `Default`;
  - in `app.rs`, se `cfg.record_calls`:
    - `start_call` → `store.begin(app, auto, now_ms())`;
    - CALL_ALIVE → `store.touch(now_ms())`;
    - `end_call` e `shutdown` → `store.end(now_ms())`;
  - alla modifica di `record_calls` (in `ConfigChanged`, confrontando con il valore precedente):
    - spento durante una call → `store.end(now)`;
    - acceso durante una call → `store.begin(...)` con app e `auto` di `office.my_call`.
  - `Office::rename` non tocca l'archivio.

- [ ] **Step 4: esegui** `cargo test` e `cargo build`. Atteso: PASS e nessun warning nuovo.

- [ ] **Step 5: verifica a mano** con `cargo run`: dal menu "Sto entrando in call", aspetta 30 s, poi "Termina la mia call". In `call.json` compare un tratto di circa 30 s con app `"Manuale"`. Ripeti con "Registra le mie call" spento (modificando `config.json` ad app chiusa): il file non cambia.

- [ ] **Step 6: commit** `feat(stats): registrazione delle proprie call`

### Task 5: finestra "Le mie call"

**Files:**
- Create: `src/statsui.rs`
- Modify: `src/main.rs` (`mod statsui;`), `src/tray.rs` (voce e `Action::OpenCallStats`), `src/app.rs` (`stats_win`, `stats_period: Period`, apertura, disegno, chiusura), `src/snapshot.rs` (`--ui statistiche`)

**Interfaces:**
- Consumes: `Report`, `Period`, `PeriodKind`, `fmt_duration` (Task 1–2), `CallStore::snapshot` (Task 3).
- Produces:
  ```rust
  pub enum StatsAction { Kind(PeriodKind), Prev, Next }
  pub fn stats_panel(ui: &mut egui::Ui, p: Period, today: NaiveDate, rep: &Report, recording: bool, live: bool) -> Option<StatsAction>;
  ```
- `app.rs`:
  - `open_call_stats(el)` funziona come `open_settings`: porta in primo piano se la finestra è già aperta, dimensioni 560×640 non ridimensionabili, titolo `DevLogica Tools – Le mie call`;
  - all'apertura `stats_period = Period::containing(Week, oggi)`;
  - in `paint_ui` la finestra è il kind 4 e il report si calcola a ogni disegno con `store.snapshot(now_ms())` e `chrono::Local`;
  - `CloseRequested` chiude la finestra come per `settings`;
  - `StatsAction::Kind(k)` porta a `Period::containing(k, oggi)`, `Prev`/`Next` si spostano, e `Next` funziona solo se `can_go_next`.
- `live = store.is_open()`: se è vero, il pannello chiama `ui.ctx().request_repaint_after(5 s)`, altrimenti non chiede ridisegni.
- Il grafico si disegna con `ui.painter()`. Il tooltip su ogni barra usa `Response::on_hover_text(bar.tooltip)` di un'area cliccabile allocata sulla barra. La barretta "Per app" è proporzionale alla prima app.
- Il menu ha la voce `Le mie call…` subito prima di `Impostazioni…`.

- [ ] **Step 1: aggiungi `statistiche` a `snapshot.rs`**, con tratti di esempio sulla settimana corrente: Teams, Slack, Meet, più una call in corso. Dimensioni 560×640.

- [ ] **Step 2: implementa** `stats_panel`, la voce di menu e i collegamenti in `app.rs`.

- [ ] **Step 3: verifica** con `cargo run -- --ui statistiche /tmp/…/stats.png` e guarda l'immagine. Deve corrispondere alla bozza A: schede in alto con Settimana attiva, totale grande, nome del periodo e frecce, media, 7 barre con quelle future più scure, sezione "Per app". Ripeti con le schede Giorno (elenco), Mese e Anno: in `snapshot.rs` basta un argomento opzionale `giorno|mese|anno`.

- [ ] **Step 4: verifica a mano** con `cargo run`. Dal menu apri "Le mie call…": le frecce funzionano e › si ferma al periodo in corso. Durante una call il totale cresce ogni circa 5 s. Con la finestra aperta e nessuna call, la CPU dell'app non sale.

- [ ] **Step 5: esegui** `cargo test` e `cargo clippy --all-targets`. Atteso: verde e nessun warning nuovo.

- [ ] **Step 6: commit** `feat(stats): finestra Le mie call`

### Task 6: sezione "Le mie call" nelle Impostazioni e cancellazione dello storico

**Files:**
- Modify: `Cargo.toml` (`egui_extras` 0.36 con default-features off e feature `datepicker`; verifica che la versione sia compatibile con egui 0.36), `src/ui.rs` (`View`, `UiAction`, sezione in `settings`), `src/app.rs` (`handle_ui`), `src/snapshot.rs` (dati per `View`), `README.md`

**Interfaces:**
- Consumes: `CallStore::{preview_delete, delete_range, first_day}` (Task 3), `fmt_duration` (Task 1), `open_call_stats` (Task 5).
- Produces:
  ```rust
  // ui.rs
  pub struct DeleteForm { pub from: NaiveDate, pub to: NaiveDate, pub open: bool, pub confirm: Option<(usize, f64)> }
  // in View: pub delete: &'a mut DeleteForm, pub first_call_day: Option<NaiveDate>
  // UiAction: OpenCallStats, PreviewDelete, DeleteCalls
  ```
- Nella sezione `Le mie call`, subito dopo "Avvisi delle call dei colleghi", ci sono:
  - il pulsante `Le mie call…`, che emette `OpenCallStats`;
  - la casella `Registra le mie call`, che emette `ConfigChanged` come le altre;
  - il pulsante `Cancella lo storico…`, che apre il modulo con:
    - `Dal` e `Al`, due `DatePickerButton` (di default `Dal` = primo giorno del mese corrente e `Al` = oggi, e se `Dal > Al` si scambiano);
    - `Tutto`, che imposta `Dal = first_call_day` e `Al = oggi`, ed è disattivato se non ci sono call;
    - `Cancella`, che emette `PreviewDelete`.
- L'app risponde a `PreviewDelete` riempiendo `confirm` con `store.preview_delete(...)`. Il modulo mostra allora il testo di conferma con i pulsanti `Cancella` (che emette `DeleteCalls`) e `Annulla`.
  - Con 0 call da cancellare, al posto della conferma compare `Nessuna call in questo intervallo`.
  - `DeleteCalls` esegue `store.delete_range(...)`, chiude il modulo e ridisegna la finestra delle statistiche se è aperta.
- Il testo di conferma usa `fmt_duration` per la durata e la forma estesa per le date, per esempio `1° ottobre 2026`. Se le due date sono nello stesso mese: `dal 1° al 31 ottobre 2026`.

- [ ] **Step 1: scrivi il test** in `callstats.rs`: `testo_conferma`.
  - `confirm_text(37, 78_000_000.0, 1/10/2026, 31/10/2026)` == `"Cancellare 37 call (21 h 40 min) dal 1° al 31 ottobre 2026?"`.
  - Con mesi diversi: `"… dal 28 settembre al 3 ottobre 2026?"`.
  - Con anni diversi: `"… dal 30 dicembre 2025 al 2 gennaio 2026?"`.
  - Firma: `pub fn confirm_text(calls: usize, ms: f64, from: NaiveDate, to: NaiveDate) -> String` in `callstats.rs`.

- [ ] **Step 2: esegui** `cargo test callstats`. Atteso: FAIL in compilazione.

- [ ] **Step 3: implementa** `confirm_text`, la sezione delle Impostazioni e la gestione delle tre `UiAction` in `app.rs`.

- [ ] **Step 4: verifica** con `cargo run -- --ui impostazioni …png`: la sezione compare con i testi esatti. Poi `cargo run`: cancella un intervallo che comprende una call di prova e controlla che sparisca dalla finestra "Le mie call" e da `call.json`.

- [ ] **Step 5: aggiorna il README** con una sezione `### Le mie call` sotto "Come si usa": cosa viene registrato, dove sta il file (solo sul PC), l'interruttore, la cancellazione per intervallo, e il fatto che la sospensione non conta. Nella *Struttura* aggiungi `callstats.rs`, `callstore.rs` e `statsui.rs`.

- [ ] **Step 6: esegui** `cargo test` e `cargo clippy --all-targets`. Atteso: verde.

- [ ] **Step 7: commit** `feat(stats): interruttore e cancellazione dello storico nelle Impostazioni`
