# Aggiornamenti dall'app e dimensione degli indicatori (v1.0.4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** aggiornare l'app con un clic (dal menu, dalle Impostazioni o dalla notifica), controllare gli aggiornamenti a mano, e regolare separatamente la dimensione dei testi e del logo degli indicatori.

**Architecture:**
- **Logica pura con test**: scelta dell'asset dalla risposta di GitHub, regola "una notifica per versione", percorsi di sostituzione, testi del menu per ogni stato, scale degli indicatori.
- **Thread separati** per controllo e scaricamento con `curl`; comunicano con l'app tramite `UserEvent`.
- **Sostituzione uguale sui due sistemi**:
  1. il file nuovo viene scaricato e verificato accanto all'app;
  2. l'app in uso viene **rinominata** (sia macOS sia Windows lo permettono a processo aperto) e quella nuova prende il suo posto;
  3. la nuova parte con `--dopo-aggiornamento` e aspetta che la porta dell'istanza unica si liberi;
  4. la vecchia si chiude, e la nuova cancella i resti.
- **Notifiche in `src/notify.rs`**:
  - macOS: `UNUserNotificationCenter` con un delegate per il clic; se non è autorizzata, ripiega su `osascript` senza clic.
  - Windows: balloon con un'icona di notifica temporanea di proprietà dell'app, e clic tramite `NIN_BALLOONUSERCLICK`.

**Tech Stack:** Rust 1.95+, winit 0.30, egui 0.36, objc2 0.6 (più `block2` e `objc2-user-notifications` su macOS), windows 0.62 (`Win32_UI_Shell`, già presente), `curl`, `ditto` e `codesign` di sistema.

**Spec:** `docs/superpowers/specs/2026-11-10-aggiornamenti-e-dimensioni-design.md`

## Global Constraints

- Nessuna libreria HTTPS: scaricamento con `curl -L -f -s`, su Windows con `CREATE_NO_WINDOW`.
- L'URL dell'asset si prende solo da `assets[].browser_download_url` di `releases/latest`.
- Nomi degli asset: `DevLogica-Tools-macOS.zip` (contiene `DevLogica Tools.app`) e `DevLogica-Tools-Windows.exe`.
- La copia in uso non si sovrascrive mai: si rinomina, e solo dopo che il file nuovo è completo e verificato. Se qualcosa va storto si ripristina il nome originale e l'app continua a funzionare.
- Istanza unica sulla porta 47824: con `--dopo-aggiornamento` si riprova per 30 s; senza, si esce subito come oggi.
- La notifica automatica compare una sola volta per versione (`notified_version` in `config.json`). Il controllo manuale la mostra sempre e non aggiorna `notified_version`.
- Testi delle notifiche: titolo `DevLogica Tools`.
  - Corpo con clic: `È disponibile la versione vX.Y.Z. Clicca per aggiornare.`
  - Corpo senza clic: `È disponibile la versione vX.Y.Z. Apri il menu per aggiornare.`
  - Nessuna novità: `Hai già l'ultima versione (X.Y.Z).`
  - Errore: `Aggiornamento non riuscito: <motivo>`.
- Testi del menu: `Cerca aggiornamenti`, `Controllo in corso…`, `Aggiorna alla vX.Y.Z`, `Download in corso…`, `Installazione…`, `Apri la pagina della release`.
- `DEVLOGICA_FAKE_VERSION`, solo per lo sviluppo: sostituisce la versione corrente nei confronti.
- Dimensioni: `text_scale` e `logo_scale` vanno da 0.75 a 2.0, default 1.0, a passi di 0.05. I margini restano 16 pt e 12 pt.
- La versione diventa 1.0.4. Testi e commenti in italiano.

## Review Focus

1. **Doppio clic, oppure clic sulla notifica mentre l'aggiornamento è in corso**: deve partire un solo aggiornamento. Test `stato_non_riparte` (Task 5).
2. **App non in un bundle** (`cargo run`) **o cartella non scrivibile**: errore chiaro, app intatta. Test `bundle_da_exe_*` (Task 3) e prova manuale (Task 6).
3. **Asset mancante, o dimensione diversa da quella dichiarata**: nessuna sostituzione. Test `scegli_asset_*` e `verifica_dimensione` (Tasks 2, 3).
4. **Riavvio con la vecchia copia ancora aperta**: la nuova aspetta e poi parte, senza mai due copie insieme né nessuna. Prova manuale (Task 6).
5. **Testi al 200% su uno schermo piccolo**: gli elementi vengono omessi interi e niente esce dallo schermo. Test `scale_*` (Task 1) e snapshot (Task 1).

---

### Task 1: Dimensione di testi e logo

**Files:** Modify `src/config.rs`, `src/overlay.rs`, `src/ui.rs` (indicators section), `src/snapshot.rs` (`--testo <s>` and `--logo <s>`)

**Interfaces:**
- Produces: in `Indicators`, `pub text_scale: f32` and `pub logo_scale: f32` (default 1.0); `impl Indicators { pub fn text_k(&self) -> f32; pub fn logo_k(&self) -> f32 }` (values clamped to 0.75..=2.0, NaN → 1.0).

- [ ] **Step 1: Failing tests** in `config.rs`:
  - `scale_default_e_vecchio_config`: `{"indicators":{"cpu":"hidden"}}` → `text_scale == 1.0`, `logo_scale == 1.0`.
  - `scale_limitate`: `text_scale: 5.0` → `text_k() == 2.0`; `0.1` → `0.75`; `NaN` → `1.0`.
- [ ] **Step 2:** `cargo test config::` → FAIL. **Step 3:** implement → PASS.
- [ ] **Step 4:** In `overlay.rs`, multiply every font size, the 1 pt gap between stack rows, `PILL_PAD` and the pill radius by `text_k()`. Multiply `LOGO_H` by `logo_k()`. The gap between blocks stays the same.
- [ ] **Step 5:** In `ui.rs`, after the position grid, add two sliders: `"Dimensione dei testi"` and `"Dimensione del logo"`, range `0.75..=2.0`, step 0.05, value shown as a percentage (`custom_formatter`, "150%"). Add a `"Ripristina"` button that sets both back to 1.0.
- [ ] **Step 6:** Snapshot options `--testo 2 --logo 2` for `text`, `pills` and `big_clock` at 3024×1964, plus `--testo 2` at 1440×900 (small screen). Expected: everything bigger and in proportion, and on the small screen whole elements omitted, nothing cut off or overlapping. `cargo run --release -- --ui impostazioni` shows the two sliders.
- [ ] **Step 7:** commit `feat: dimensione di testi e logo degli indicatori`.

### Task 2: Controllo aggiornamenti (logica)

**Files:** Modify `src/update.rs`, `src/config.rs` (`pub notified_version: String`, default `""`)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq)] pub struct Asset { pub url: String, pub size: u64 }
  #[derive(Clone, Debug, PartialEq)] pub struct Release { pub tag: String, pub page: String, pub asset: Option<Asset> }
  pub const ASSET_NAME: &str;                                  // per sistema
  pub fn current_version() -> String;                          // DEVLOGICA_FAKE_VERSION o CARGO_PKG_VERSION
  pub fn parse_release(json: &str, asset_name: &str) -> Option<Release>;
  pub fn fetch() -> Result<Option<Release>, String>;           // Ok(None) = già aggiornato
  pub fn should_notify(manual: bool, tag: &str, notified: &str) -> bool;
  ```
  `check()` stays as a thin wrapper over `fetch()` (same signature) until Task 5 removes it, so the build stays green.

- [ ] **Step 1: Failing tests:**
  - `scegli_asset_presente`: a JSON with two assets → `Asset { url, size }` taken from the one named `asset_name`.
  - `scegli_asset_mancante`: `asset: None`, `tag` still read.
  - `json_non_valido`: `parse_release("{", ..) == None`.
  - `notifica_una_volta`: `should_notify(false, "v1.0.5", "v1.0.5") == false`, `(false, "v1.0.5", "v1.0.4") == true`, `(true, "v1.0.5", "v1.0.5") == true`.
  - The existing `versioni` test stays.
- [ ] **Step 2:** `cargo test update::` → FAIL. **Step 3:** implement. `fetch` uses `curl` as now; a non-JSON reply or a reply without `tag_name` gives `Err("risposta di GitHub non valida")`; `is_newer` is checked against `current_version()`. → PASS.
- [ ] **Step 4:** commit `feat(update): release, asset e regola delle notifiche`.

### Task 3: Scaricamento, verifica e sostituzione

**Files:**
- Create: `src/install.rs`
- Modify: `src/main.rs` (`--dopo-aggiornamento`: retry the port bind for 30 s; at startup, call `install::cleanup()`)

**Interfaces:**
- Produces:
  ```rust
  pub fn bundle_from_exe(exe: &Path) -> Option<PathBuf>;   // macOS: .../X.app/Contents/MacOS/bin → .../X.app
  pub fn swap_paths(target: &Path) -> (PathBuf, PathBuf);  // (copia vecchia, file nuovo) accanto al target: "<nome>.vecchia", "<nome>.nuova"
  pub fn size_ok(path: &Path, expected: u64) -> bool;
  pub fn install(asset: &Asset, tag: &str, progress: impl Fn(Phase)) -> Result<(), String>; // scarica, verifica, scambia, rilancia; Ok = l'app deve uscire
  #[derive(Clone, Copy, Debug, PartialEq)] pub enum Phase { Downloading, Installing }
  pub fn cleanup();                                         // cancella "<nome>.vecchia"/"<nome>.nuova" accanto all'app in uso
  ```

- [ ] **Step 1: Failing tests:**
  - `bundle_da_exe_app`: `/Applications/DevLogica Tools.app/Contents/MacOS/devlogica-tools` → `Some("/Applications/DevLogica Tools.app")`.
  - `bundle_da_exe_cargo`: `target/release/devlogica-tools` → `None`.
  - `percorsi_scambio`: `"/x/App.app"` → `("/x/App.app.vecchia", "/x/App.app.nuova")`; `C:\t\DT.exe` → `DT.exe.vecchia`, `DT.exe.nuova`.
  - `verifica_dimensione`: a temporary 10-byte file → `size_ok(p, 10)` true, `size_ok(p, 11)` false.
- [ ] **Step 2:** `cargo test install::` → FAIL. **Step 3:** implement the pure functions → PASS.
- [ ] **Step 4:** Implement `install`.
  - **macOS**: target `bundle_from_exe(current_exe)`, otherwise `Err("l'app non è installata come .app")`.
    1. `curl -L -f -s -m 600 -o <tmp>.zip <url>`, then `size_ok`.
    2. `ditto -x -k <zip> <dir>`, giving `<dir>/DevLogica Tools.app`.
    3. `codesign --verify --deep`, and check that `defaults read <app>/Contents/Info CFBundleShortVersionString` equals `tag` without the `v`.
    4. `xattr -dr com.apple.quarantine`.
    5. `mv <dir>/X.app <target>.nuova`, `rename(target → .vecchia)`, `rename(.nuova → target)`.
    6. `open -n <target> --args --dopo-aggiornamento`.
    If step 5 fails, `.vecchia` is renamed back to its original name.
  - **Windows**: target `current_exe`.
    1. `curl -o <target>.nuova`, then `size_ok`.
    2. `rename(target → .vecchia)`, `rename(.nuova → target)`, otherwise roll back.
    3. `Command::new(target).arg("--dopo-aggiornamento")` with `CREATE_NO_WINDOW | DETACHED_PROCESS`.
  - Temporary files go in `config::app_dir()/aggiornamento` (macOS) and are deleted when done. Every step and error goes to `log`.
- [ ] **Step 5:** In `main.rs`, with `--dopo-aggiornamento` the bind is retried every 250 ms for up to 30 s, and the log gets `"Aggiornato alla versione X"`. `cleanup()` runs at every startup, ignoring errors (Windows may keep `.vecchia` locked for a moment, so it retries on the next startup).
- [ ] **Step 6:** commit `feat: scaricamento e sostituzione dell'app`.

### Task 4: Notifiche di sistema con clic

**Files:**
- Create: `src/notify.rs`
- Modify: `Cargo.toml` (macOS: `block2 = "0.6"`, `objc2-user-notifications = { version = "0.3", features = [...] }`), `src/platform/windows.rs` (balloon)

**Interfaces:**
- Produces:
  ```rust
  pub fn init(on_click: impl Fn() + Send + Sync + 'static);  // registra il gestore del clic (manda UserEvent::NotificationClicked)
  pub fn clickable() -> bool;                                 // vero se il clic arriva davvero su questo sistema
  pub fn show(body: &str, click_updates: bool);               // titolo fisso "DevLogica Tools"
  ```

- [ ] **Step 1 (spike, throwaway): macOS.** In a built `.app`, check that `UNUserNotificationCenter currentNotificationCenter` works with ad-hoc signing and `Accessory`: `requestAuthorization`, a notification, and the click reaching the delegate. Write the outcome in the ledger. If it does not work, `clickable()` is false on macOS and `show` uses `osascript -e 'display notification ...'`.
- [ ] **Step 2:** macOS implementation. Call `currentNotificationCenter` only if `NSBundle.mainBundle.bundleIdentifier` is not nil; with `cargo run`, use the `osascript` fallback. The delegate is a class defined with `define_class!`, and `didReceiveNotificationResponse` calls `on_click` only for notifications whose `userInfo` contains `"aggiorna"`. Add `willPresentNotification`, so the notification shows even while the app is active.
- [ ] **Step 3:** Windows implementation.
  - A hidden window of its own (`RegisterClassW` + `CreateWindowExW` with `HWND_MESSAGE`) and a temporary notify icon (`NIM_ADD` with `uID = 7`, the app icon, `NIF_INFO`, `szInfoTitle` / `szInfo`).
  - On `NIN_BALLOONUSERCLICK` it calls `on_click`; on `NIN_BALLOONTIMEOUT` or `NIN_BALLOONHIDE` it removes the icon (`NIM_DELETE`).
  - winit's message pump dispatches the messages on the same thread. `clickable()` is true.
- [ ] **Step 4:** Manual check: an `--notifica-prova` command-line option shows a clickable notification and logs the click. On macOS, try it from the `.app`; on Windows, the user checks it from the CI exe. Remove the option before the final commit, or keep it as a diagnostic tool documented in the README (*Strumenti utili*).
- [ ] **Step 5:** commit `feat: notifiche di sistema con clic`.

### Task 5: Stati, menu, Impostazioni e collegamento

**Files:** Modify `src/update.rs` (`UpdateState`), `src/app.rs`, `src/tray.rs`, `src/ui.rs`, `src/snapshot.rs`

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Debug, PartialEq)]
  pub enum UpdateState { Idle, Checking { manual: bool }, Available(Release), Downloading(Release), Installing(Release), Failed { release: Release, reason: String } }
  impl UpdateState {
      pub fn menu_items(&self) -> Vec<(String, bool, Option<UpdateCmd>)>;  // (testo, attiva, comando)
      pub fn can_start(&self) -> bool;                                       // solo Available / Failed con asset
  }
  #[derive(Clone, Copy, Debug, PartialEq)] pub enum UpdateCmd { Check, Install, OpenPage }
  // app.rs: UserEvent::{CheckDone { manual: bool, result: Result<Option<Release>, String> }, InstallPhase(Phase), InstallFailed(String), NotificationClicked}
  ```

- [ ] **Step 1: Failing tests** in `update.rs`:
  - `menu_idle`: `[("Cerca aggiornamenti", true, Some(Check))]`.
  - `menu_checking`: `[("Controllo in corso…", false, None)]`.
  - `menu_available`: `[("Aggiorna alla v1.0.5", true, Some(Install)), ("Cerca aggiornamenti", true, Some(Check))]`.
  - `menu_downloading`: `("Download in corso…", false, None)`, and Check is disabled.
  - `menu_failed`: `Aggiorna alla v1.0.5`, `Apri la pagina della release`, `Cerca aggiornamenti`.
  - `menu_available_senza_asset`: only `Apri la pagina della release` and Check.
  - `stato_non_riparte`: `can_start()` is false for `Downloading` and `Installing`.
- [ ] **Step 2:** `cargo test update::` → FAIL. **Step 3:** implement → PASS.
- [ ] **Step 4:** `app.rs`:
  - replace `update: Option<(String, String)>` with `upd: UpdateState`;
  - the automatic thread sends `CheckDone { manual: false }`; `Action::CheckUpdates` starts a one-off thread with `manual: true`;
  - on `CheckDone`:
    - with a new version → `Available`, and `notify::show(...)` if `should_notify`; when automatic, also save `notified_version`;
    - with no new version and `manual` → `"Hai già l'ultima versione (X)"`;
    - `Err` with `manual` → error notification;
  - `Action::InstallUpdate` and `NotificationClicked` act only if `can_start()`; they move to `Downloading` and start `install::install` in a thread that sends `InstallPhase` / `InstallFailed`; on `Ok`, `shutdown()` and `el.exit()`;
  - `InstallFailed` → `Failed` and an error notification;
  - `notify::init` in `resumed`, with the proxy.
- [ ] **Step 5:** In `tray.rs`, `MenuState.update: &UpdateState` replaces `Option<&str>`. Its `menu_items()` come before "Versione X", mapped to `Action::{CheckUpdates, InstallUpdate, OpenUpdate}`. The commands do not appear if `update::repo()` is `None`. In `ui.rs`, the Settings header shows the same items as a link or button (`UiAction::{CheckUpdates, InstallUpdate, OpenUpdate}`). `refresh_tray`'s signature includes the state.
- [ ] **Step 6:** `cargo build`, `cargo test` → green, no warnings. `--ui impostazioni` with `Available` → the button can be seen.
- [ ] **Step 7:** commit `feat: aggiornamento a un clic da menu, Impostazioni e notifica`.

### Task 6: Prova completa, versione, README

**Files:** Modify `Cargo.toml` (1.0.4), `README.md`

- [ ] **Step 1: macOS dry run.**
  1. `./packaging/macos/build-app.sh` with `DEVLOGICA_REPO=egodi99/devlogica-tools`, then copy it to a test folder (`~/Applications-prova/`).
  2. Start it with `DEVLOGICA_FAKE_VERSION=1.0.0`. The user's 1.0.2/1.0.3 must be closed: ask first.
  3. Expected:
     - notification "È disponibile la versione v1.0.3";
     - the click starts the update; the menu shows "Download in corso…" and then the app restarts;
     - the bundle in the test folder is now 1.0.3 (`CFBundleShortVersionString`);
     - `.vecchia` is removed;
     - the log contains "Aggiornato alla versione".
- [ ] **Step 2: Error dry run.** Use a read-only test folder (`chmod a-w`). Expected: "Aggiornamento non riuscito: …", the app keeps running, and the menu offers "Aggiorna" again plus "Apri la pagina della release".
- [ ] **Step 3: Windows.** It can't be tried here. The pipeline builds the exe; ask the user to run the same dry run on a Windows PC with `set DEVLOGICA_FAKE_VERSION=1.0.0`. Write the outcome in the ledger.
- [ ] **Step 4:** README:
  - *Installazione*: from 1.0.4, "Aggiorna" from the menu;
  - *Come si usa → Aggiornamenti*: check, notification and errors;
  - *Indicatori*: the two sizes and the Accessibilità note after every update;
  - *Pubblicare una nuova versione*: the asset names must not change, otherwise in-app updating breaks;
  - *Strumenti utili*: `DEVLOGICA_FAKE_VERSION`.
  Set `Cargo.toml` to 1.0.4.
- [ ] **Step 5:** `cargo test`, `cargo build --release`, no warnings. Commit `release: 1.0.4 – aggiornamento a un clic e dimensione indicatori`. Don't create the tag: the user publishes it.
