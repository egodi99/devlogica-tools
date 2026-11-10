# Indicatori sullo sfondo (v1.0.3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** su macOS, disegnare CPU, RAM, orologio e logo DevLogica dentro lo sfondo animato, ai lati del Dock, solo sullo schermo che ha il Dock, con posizione per elemento e tre stili configurabili.

**Architecture:** la logica pura (formattazione, ordine e blocchi per lato, spazio disponibile, omissione, calcolo CPU e RAM) sta in moduli multipiattaforma con test unitari. macOS fornisce i dati: Mach/sysctl per CPU e RAM, `localtime_r` per l'ora, Accessibilità per il rettangolo del Dock con fallback su `NSScreen`. Gli indicatori si disegnano con un secondo passaggio egui sulla stessa superficie dello sfondo, nello stesso fotogramma, subito dopo lo shader. Gli shader non cambiano.

**Tech Stack:** Rust 1.95+, winit 0.30, wgpu 30 (Metal), egui/egui-wgpu 0.36, objc2 0.6, framework ApplicationServices (AX) via `extern "C"`. Nessuna dipendenza nuova.

**Spec:** `docs/superpowers/specs/2026-11-10-indicatori-sfondo-design.md`

## Global Constraints

- Solo macOS. Su Windows non si disegna niente, e la sezione Impostazioni e la voce di menu non compaiono. I tipi di configurazione e la logica pura compilano ovunque, così i test girano anche nella CI Windows.
- Gli shader degli sfondi (`shaders/*.wgsl`, `prelude.wgsl`) e il contratto `scene()` / `logo_layout()` restano invariati.
- Statistiche e misura del Dock mai a ogni fotogramma: ogni ~2 s. A ogni fotogramma solo valori già letti.
- `Config` resta `#[serde(default)]`: un `config.json` della 1.0.2 si carica e ottiene i default nuovi.
- Default: indicatori attivi, stile A (solo testo), logo e orologio a sinistra, RAM e CPU a destra.
- Formati: `CPU 23%`, `RAM 11,2 / 16 GB` (una cifra decimale per l'usata, virgola italiana, totale intero), orologio `HH:MM` nel formato 24 ore, ora locale.
- Ordine su ogni lato, dal bordo dello schermo verso il Dock: logo → orologio → RAM → CPU.
- Margine dal bordo dello schermo 16 pt. Distanza minima dal Dock 12 pt. Spazio tra blocchi 14 pt (6 pt nello stile B).
- Senza permesso Accessibilità, o con il Dock nascosto: fascia centrale libera pari al 70% della larghezza dello schermo.
- Dissolvenza di comparsa uguale al logo grande: opacità `smoothstep(1.3, 2.6, since)`.
- Il permesso Accessibilità si chiede (con prompt di sistema) solo su macOS, solo se gli indicatori sono attivi, al massimo una volta per avvio.
- Testi dell'interfaccia e commenti in italiano.
- Versione 1.0.3 in `Cargo.toml`; release con il tag `v1.0.3`.

## Review Focus

1. **Dock che cambia larghezza** (un'app si apre o si chiude): entro ~2 s nessun indicatore si sovrappone. La logica è testata da `fit_*` (Task 2). Va verificata a mano nel Task 6.
2. **Lato troppo stretto** (schermo piccolo, Dock pieno): gli elementi più vicini al Dock spariscono interi, non si tagliano e non si sovrappongono. Test `fit_omette_dal_dock` (Task 2).
3. **Permesso negato o revocato mentre l'app gira:** passaggio silenzioso alla fascia fissa, senza crash e senza nuovi prompt. Test `free_widths_senza_dock` (Task 2), più la verifica manuale nel Task 6.
4. **Qualità "Ridotta" (scale 0,5) e schermi Retina:** il testo resta leggibile, e i ppp tengono conto di `scale_factor × target.scale`. Verifica con lo snapshot del Task 5 a `--scale 0.5`.
5. **Valori limite delle statistiche:** CPU al primo campione senza delta, contatori invariati, RAM a 0 o oltre il totale. Mai NaN, mai oltre 100%. Test `cpu_percent_*` e `ram_used_*` (Task 3).

---

### Task 1: Tipi di configurazione

**Files:**
- Modify: `src/config.rs` (nuovi tipi accanto a `AlertStyle`, campo in `Config`, `Default`)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
  pub enum IndicatorStyle { Text, Pills, BigClock }          // A, B, C
  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")]
  pub enum Side { Hidden, Left, Right }
  #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)] #[serde(default)]
  pub struct Indicators { pub enabled: bool, pub style: IndicatorStyle, pub logo: Side, pub clock: Side, pub ram: Side, pub cpu: Side }
  // in Config:  pub indicators: Indicators,
  ```

- [ ] **Step 1: Write the failing tests** in a `#[cfg(test)] mod tests` in `src/config.rs`
  - `indicatori_default`: `Config::default().indicators == Indicators { enabled: true, style: Text, logo: Left, clock: Left, ram: Right, cpu: Right }`.
  - `config_vecchio_si_carica`: `serde_json::from_str::<Config>(r#"{"name":"Anna","show_logo":false}"#)` → `name == "Anna"`, `show_logo == false`, `indicators == Indicators::default()`.
  - `indicatori_parziali`: `{"indicators":{"cpu":"hidden"}}` → `cpu == Hidden`, the other fields at their defaults.
- [ ] **Step 2:** `cargo test config::` → FAIL (types missing).
- [ ] **Step 3:** Implement the types, `impl Default for Indicators`, and the field in `Config` and in `Config::default()`.
- [ ] **Step 4:** `cargo test config::` → PASS. `cargo build` without warnings.
- [ ] **Step 5:** `git commit -am "feat(config): impostazioni degli indicatori sullo sfondo"`

### Task 2: Logica pura degli indicatori

**Files:**
- Create: `src/indicators.rs` (registered in `main.rs` with `mod indicators;`)

**Interfaces:**
- Consumes: `Indicators`, `IndicatorStyle`, `Side` (Task 1).
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq, Eq)] pub enum Item { Logo, Clock, Ram, Cpu }
  #[derive(Clone, Debug, PartialEq, Eq)] pub enum Block { Single(Item), ClockStack(Vec<Item>) } // ClockStack: orologio grande + righe piccole sotto (stile C)
  pub fn blocks(ind: &Indicators, side: Side) -> Vec<Block>          // ordine: dal bordo schermo verso il Dock
  /// Dock in coordinate locali dello schermo, in punti.
  #[derive(Clone, Copy, Debug, PartialEq)] pub enum DockPlace { Bottom { x0: f32, x1: f32, top: f32 }, Side, Unknown }
  pub fn free_widths(screen_w: f32, dock: DockPlace) -> (f32, f32)  // larghezza utile (sinistra, destra), già al netto dei margini
  pub fn fit(widths: &[f32], available: f32, gap: f32) -> usize     // quanti blocchi (dall'esterno) ci stanno
  pub fn fmt_cpu(pct: f32) -> String
  pub fn fmt_ram(used: u64, total: u64) -> String
  pub fn fmt_clock(h: u32, m: u32) -> String
  pub const EDGE_MARGIN: f32 = 16.0; pub const DOCK_MARGIN: f32 = 12.0; pub const FALLBACK_BAND: f32 = 0.70;
  ```

- [ ] **Step 1: Write the failing tests** in `src/indicators.rs`
  - `blocks_ordine`: default settings, style Text → `blocks(.., Left) == [Single(Logo), Single(Clock)]`, `blocks(.., Right) == [Single(Ram), Single(Cpu)]`.
  - `blocks_tutti_a_sinistra`: all on Left → `[Logo, Clock, Ram, Cpu]` as `Single`s. `Right` → `[]`.
  - `blocks_nascosti`: `enabled: false` → both sides `[]`. `cpu: Hidden` → Cpu appears on neither side.
  - `blocks_stile_c`: BigClock, clock+ram+cpu Left, logo Left → `[Single(Logo), ClockStack(vec![Ram, Cpu])]`. Clock Left, cpu Right → Right `== [Single(Cpu)]`. Clock Hidden → same result as style Text.
  - `free_widths_dock_in_basso`: `screen_w 1512, Bottom{x0: 400, x1: 1112, ..}` → `(400-16-12, 1512-1112-16-12) == (372.0, 372.0)`.
  - `free_widths_senza_dock`: `Unknown` → each side `1512*(1-0.70)/2 - 16 - 12`. `Side` → each side `1512/2 - 16`.
  - `free_widths_mai_negativo`: Dock wider than the screen → `(0.0, 0.0)`.
  - `fit_tutto`: `fit(&[50.,80.], 200., 14.) == 2`. `fit_omette_dal_dock`: `fit(&[50.,80.,90.], 160., 14.) == 2`. Second element too wide → `fit(&[50.,500.,10.], 160., 14.) == 1` (stops at the first one that doesn't fit). `fit(&[], 0., 14.) == 0`.
  - `formati`: `fmt_cpu(23.4) == "CPU 23%"`. `fmt_cpu(f32::NAN) == "CPU 0%"`. `fmt_cpu(140.) == "CPU 100%"`. `fmt_ram(12_025_908_428, 17_179_869_184) == "RAM 11,2 / 16 GB"` (GiB, labelled GB like Activity Monitor). `fmt_clock(9, 5) == "09:05"`.
- [ ] **Step 2:** `cargo test indicators::` → FAIL.
- [ ] **Step 3:** Implement. `fit` adds the widths, plus `gap` between consecutive blocks, and stops at the first block that would exceed `available`. `free_widths` clamps to ≥ 0.
- [ ] **Step 4:** `cargo test indicators::` → PASS.
- [ ] **Step 5:** `git commit -m "feat: logica degli indicatori (ordine, spazio, formati)"`

### Task 3: Statistiche di sistema e ora locale

**Files:**
- Create: `src/sysstats.rs` (pure part on every platform, sampler on macOS)
- Modify: `src/platform/macos.rs` (FFI: `host_statistics`/`HOST_CPU_LOAD_INFO`, `host_statistics64`/`HOST_VM_INFO64`, `sysctlbyname("hw.memsize")`, `localtime_r`), `src/platform/mod.rs` (stubs `None` for Windows/other)

**Interfaces:**
- Produces:
  ```rust
  #[derive(Clone, Copy, Debug, Default, PartialEq)] pub struct Stats { pub cpu: f32, pub ram_used: u64, pub ram_total: u64 }
  pub fn cpu_percent(prev: [u64; 4], cur: [u64; 4]) -> f32           // tick [user, system, idle, nice]
  pub fn ram_used(internal: u64, purgeable: u64, wired: u64, compressed: u64, page: u64) -> u64
  pub struct Sampler { /* Arc<Mutex<Stats>>, thread */ }
  impl Sampler { pub fn start() -> Self; pub fn latest(&self) -> Stats }   // thread "statistiche", campione ogni 2 s
  // platform (macOS reale, altrove None):
  pub fn cpu_ticks() -> Option<[u64; 4]>;
  pub fn vm_pages() -> Option<(u64, u64, u64, u64, u64)>;  // internal, purgeable, wired, compressor, page_size
  pub fn mem_total() -> Option<u64>;
  pub fn local_hm() -> (u32, u32);
  ```

- [ ] **Step 1: Write the failing tests** in `src/sysstats.rs`
  - `cpu_percent_base`: `prev [100,50,850,0]`, `cur [130,70,1000,0]` → `(30+20)/(200) = 25.0`.
  - `cpu_percent_nessun_delta`: `prev == cur` → `0.0`. Counters going backwards (wraparound) → `0.0`, never NaN.
  - `ram_used_base`: `ram_used(1000, 100, 200, 50, 16384) == (1000-100+200+50)*16384`. `purgeable > internal` → no underflow (saturating).
- [ ] **Step 2:** `cargo test sysstats::` → FAIL.
- [ ] **Step 3:** Implement the pure functions, the macOS FFI and the `Sampler`. The thread keeps the previous ticks, and the first sample publishes `cpu: 0`. `ram_used` follows Activity Monitor's "Memoria utilizzata": app memory + wired + compressed.
- [ ] **Step 4:** `cargo test sysstats::` → PASS. On a Mac, add a temporary `#[ignore]` test that prints `Sampler::start().latest()` after 2.5 s. `cargo test sysstats -- --ignored --nocapture` → values close to Activity Monitor (CPU within ±10 points, RAM within ±1 GB). Then remove the print test.
- [ ] **Step 5:** `git commit -m "feat: statistiche CPU/RAM e ora locale su macOS"`

### Task 4: Posizione del Dock e permesso Accessibilità

**Files:**
- Modify: `src/platform/macos.rs` (link `ApplicationServices`: `AXIsProcessTrustedWithOptions`, `AXUIElementCreateApplication`, `AXUIElementCopyAttributeValue`, `AXValueGetValue`; PID of the Dock via `NSRunningApplication runningApplicationsWithBundleIdentifier:@"com.apple.dock"`; `NSScreen screens` with `frame`/`visibleFrame`), `src/platform/mod.rs` (stubs)

**Interfaces:**
- Produces (platform):
  ```rust
  #[derive(Clone, Copy, Debug, PartialEq)] pub struct GRect { pub x: f64, pub y: f64, pub w: f64, pub h: f64 } // punti globali, origine in alto a sinistra dello schermo principale (come AX)
  pub struct DockInfo { pub screen: GRect, pub dock: Option<GRect>, pub at_bottom: bool }
  pub fn ax_trusted(prompt: bool) -> bool;
  pub fn dock_info() -> Option<DockInfo>;   // None fuori da macOS
  pub fn open_accessibility_settings();     // x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility
  ```
- Produces (`src/indicators.rs`): `pub fn dock_place(info_screen: GRect, dock: Option<GRect>, at_bottom: bool) -> DockPlace` (converts to coordinates local to the screen).

- [ ] **Step 1: Write the failing tests** in `src/indicators.rs`: `dock_place_in_basso` (screen `{0,0,1512,982}`, dock `{400,920,712,58}`, at_bottom → `Bottom{x0:400,x1:1112,top:920}`), `dock_place_secondo_schermo` (screen `{1512,0,1920,1080}`, dock `{2000,1010,800,64}` → `x0: 488`), `dock_place_none` (`None` → `Unknown`), `dock_place_laterale` (`at_bottom == false` → `Side`).
- [ ] **Step 2:** `cargo test indicators::dock_place` → FAIL. Step 3: implement `dock_place` → PASS.
- [ ] **Step 4:** Implement the macOS FFI. With AX: Dock app → `AXChildren` → the element with `AXRole == "AXList"` → `AXPosition`/`AXSize`. The screen is the `NSScreen` that contains the centre of the Dock, with `frame` converted to top-left origin. `at_bottom` is true when the Dock is wider than tall. Without AX, or with no AXList: the screen where `visibleFrame.origin.y > frame.origin.y` (Dock at the bottom). If none qualifies, `mainScreen` with `dock: None`. Every CF object gets a `CFRelease`.
- [ ] **Step 5:** Manual check on a Mac. A temporary `#[ignore]` test prints `dock_info()` and `ax_trusted(false)` with and without the permission granted in a terminal. Expected: a plausible rect when the permission is granted, `dock: None` and the right screen when it is not. Remove the test.
- [ ] **Step 6:** `git commit -m "feat(macos): posizione del Dock via Accessibilità con fallback"`

### Task 5: Disegno degli indicatori (overlay egui) + snapshot

**Files:**
- Create: `src/overlay.rs`
- Modify: `src/render.rs` (`Gpu::draw` and `render_offscreen` accept an extra pass), `src/snapshot.rs` (`--indicatori <text|pills|big_clock>` option)

**Interfaces:**
- Consumes: `blocks`, `fit`, `free_widths`, `fmt_*`, `DockPlace`, `EDGE_MARGIN` (Task 2); `Stats` (Task 3).
- Produces:
  ```rust
  pub struct Frame<'a> { pub ind: &'a Indicators, pub stats: Stats, pub hm: (u32, u32), pub dock: DockPlace, pub opacity: f32 }
  pub struct Overlay { /* egui::Context, egui_wgpu::Renderer, logo TextureHandle, format */ }
  impl Overlay {
      pub fn new(gpu: &Gpu, format: wgpu::TextureFormat) -> Self;
      /// Disegna sopra il contenuto già presente in `view` (LoadOp::Load). `size_px` = dimensione della superficie, `ppp` = pixel per punto della superficie.
      pub fn draw(&mut self, gpu: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, size_px: [u32; 2], ppp: f32, f: &Frame);
  }
  // render.rs
  pub fn draw(&mut self, t: &mut Target, w: &Wallpaper, params: Params, extra: Option<&mut dyn FnMut(&Gpu, &mut wgpu::CommandEncoder, &wgpu::TextureView)>);
  ```

- [ ] **Step 1:** Change `Gpu::draw` / `render_offscreen` so that, after the shader pass, they call `extra` on the same encoder and view before `submit`. Update the existing call sites by passing `None`. `cargo build`, `cargo test` → still green.
- [ ] **Step 2:** Implement `Overlay`. The egui `Context` gets `RawInput` with `screen_rect = size_px/ppp` and no input. Draw with a `Painter`: measure each block with `layout_no_wrap`, call `fit` against `free_widths`, then place the blocks from the outside. The left side uses x from `EDGE_MARGIN`. The right side mirrors it, so the logo sits on the right edge. Vertical: centred on the Dock band (`top` to the bottom of the screen) for `Bottom`, otherwise centred 16 pt + half the height from the bottom. Logo: `assets/logo.png` loaded as an egui texture with linear filtering, 18 pt tall (in style B inside its pill). Fonts and colours as in the mockup `stile-indicatori-v2.html`:
  - **A**: white at alpha 0.75, 12 pt; the "CPU"/"RAM" labels at 10 pt, alpha 0.5; clock 20 pt.
  - **B**: pill fill `white α 0.10`, stroke `white α 0.16`, radius 10, padding 10×5.
  - **C**: clock 30 pt, small rows at 11 pt, alpha 0.7, stacked under the clock and aligned towards the screen edge.
  Every alpha is multiplied by `opacity`. Render pass with `LoadOp::Load`.
- [ ] **Step 3:** Add `--indicatori <stile>` to `--snapshot`. It uses fixed data: `Stats{cpu:23.,ram_used:12_025_908_428,ram_total:17_179_869_184}`, `hm (14,32)`, `Bottom` dock centred and 47% wide, `opacity 1`, `ppp = 2 × scale`. It works with `--scale`.
- [ ] **Step 4: Visual check.** `cargo run --release -- --snapshot lame 3024 1964 20 /tmp/A.png --indicatori text`, then the same for `pills` and `big_clock`, plus `text --scale 0.5`. Open the PNGs. Expected: elements in the bottom corners, the right order, readable text, nothing in the central Dock band, and the 0.5 version still readable.
- [ ] **Step 5:** `git commit -m "feat: disegno degli indicatori sopra lo sfondo"`

### Task 6: Collegamento nell'app (solo schermo con il Dock)

**Files:**
- Modify: `src/wallpaper.rs` (`Screen` gets `global: GRect` in points; `render` takes the indicators), `src/app.rs` (Sampler, dock refresh in the 2 s tick, one-time prompt, `Overlay` per surface format)

**Interfaces:**
- Consumes: `Sampler`, `dock_info`, `ax_trusted`, `dock_place`, `Overlay`, `Frame`.
- Produces: `Wallpapers::render(&mut self, gpu: &mut Gpu, cfg: &Config, catalog: &[Wallpaper], ind: Option<&IndicatorState>)` with `pub struct IndicatorState { pub stats: Stats, pub hm: (u32,u32), pub dock: Option<DockInfo> }`. Wallpapers owns `overlays: HashMap<wgpu::TextureFormat, Overlay>`, created lazily and emptied in `rebuild` when indicators are off.

- [ ] **Step 1:** In `rebuild`, store for every screen its global rect in points: `m.position().to_logical(scale)` and `m.size().to_logical(scale)`. The y coordinate is already top-left, as in winit/AX.
- [ ] **Step 2:** In `render`, on macOS only and only when `cfg.indicators.enabled`: the target screen is the one whose `global` equals (±1 pt) `dock.screen`. Without `DockInfo`, use the screen at position (0,0). `ppp = window.scale_factor() × target.scale`, `opacity = smoothstep(1.3, 2.6, since)`, and the `Frame` goes into `gpu.draw(.., Some(&mut |g, enc, view| overlay.draw(...)))`. The other screens get `None`.
- [ ] **Step 3:** In `App`, on macOS: start `Sampler` in `resumed`. In the `next_check` tick (2 s), when indicators are enabled: `ax_trusted(prompt)` with `prompt = !self.ax_prompted`, then `self.ax_prompted = true`; then `dock_info()`. Cache the result in `self.ind_state`. Pass `self.ind_state.as_ref()` to `wall.render`.
- [ ] **Step 4:** `cargo build` on macOS and `cargo test` → green. `cargo check --target x86_64-pc-windows-msvc` if available, otherwise rely on CI. No unused-code warnings on Windows: the macOS-only code is behind `#[cfg]`.
- [ ] **Step 5: Manual check on a Mac**, with `cargo run --release` and two screens if possible:
  - The indicators appear only on the screen with the Dock.
  - Opening 5 apps widens the Dock, and within ~2 s the elements nearest the Dock get dropped instead of overlapping.
  - Revoking the Accessibilità permission switches to the fixed band, with no new prompt.
  - "In pausa" freezes them; turning the wallpapers off hides them.
  - Activity Monitor shows a CPU use for the app similar to 1.0.2.
- [ ] **Step 6:** `git commit -am "feat: indicatori sullo sfondo dello schermo con il Dock"`

### Task 7: Impostazioni e menu

**Files:**
- Modify: `src/ui.rs` (`View.ax_trusted: bool`; `UiAction::OpenAccessibility`; new section), `src/tray.rs` (`Action::ToggleIndicators`, item under "Mostra il logo"), `src/app.rs` (handling both actions, `ax_trusted` in the `View`), `src/snapshot.rs` (`ax_trusted: false` in the preview)

**Interfaces:**
- Consumes: `Indicators` (Task 1), `platform::open_accessibility_settings`, `platform::ax_trusted` (Task 4).

- [ ] **Step 1:** In `ui::settings`, after "Avvisi delle call dei colleghi" and inside `#[cfg(target_os = "macos")]`, add the section `"Indicatori sullo sfondo"`, with this copy:
  - checkbox `"Mostra CPU, RAM, orologio e logo ai lati del Dock"` → `enabled`;
  - muted line `"Fanno parte dello sfondo animato: con gli sfondi disattivati o in pausa non si vedono."`;
  - radio buttons, under `"Stile"`: `"Solo testo"`, `"Pillole come il Dock"`, `"Orologio grande"`;
  - a 4-row `egui::Grid` (`"Logo"`, `"Orologio"`, `"RAM"`, `"CPU"`) × the radio buttons `"Nascosto"` / `"Sinistra"` / `"Destra"`;
  - the line `"Posizione esatta del Dock: consentita"` or `"… non consentita"` and, when it is not, a `"Consenti in Accessibilità…"` button that pushes `UiAction::OpenAccessibility`, plus the muted line `"Senza permesso lascio libera una fascia al centro. Dopo un aggiornamento può servire ridare il permesso."`.
  Every control except the checkbox is disabled when `enabled` is false.
- [ ] **Step 2:** In `tray::build`, after "Mostra il logo", inside `#[cfg(target_os = "macos")]`, add `check("Mostra gli indicatori", cfg.indicators.enabled, Action::ToggleIndicators)`. The `ToggleIndicators` variant gets `#[cfg_attr(not(target_os = "macos"), allow(dead_code))]`.
- [ ] **Step 3:** In `app.rs`: `ToggleIndicators` flips `cfg.indicators.enabled` and saves, the same way as `ToggleLogo`. `OpenAccessibility` calls `platform::open_accessibility_settings()`. The settings window repaints every 2 s already (`poll_detector`), so the permission status updates by itself.
- [ ] **Step 4:** `cargo run --release -- --ui impostazioni /tmp/imp.png` on a Mac → the new section shows with the right copy. On Windows/CI the section does not exist. `cargo test` → green.
- [ ] **Step 5:** `git commit -am "feat: impostazioni e menu degli indicatori"`

### Task 8: Versione, documentazione, verifica finale

**Files:**
- Modify: `Cargo.toml` (`version = "1.0.3"`; `Cargo.lock` updates), `README.md` (*Sfondi animati* section: indicators, styles, Accessibilità permission and what happens without it, re-granting after updates; *Struttura*: `indicators.rs`, `sysstats.rs`, `overlay.rs`)

- [ ] **Step 1:** Update `Cargo.toml` and the README.
- [ ] **Step 2:** `cargo test` and `cargo build --release` → green, no warnings. `./packaging/macos/build-app.sh universal` → the `.app` builds.
- [ ] **Step 3:** Start the `.app` built in Step 2 from `/Applications`, grant the permission, and check that the indicators follow the Dock. Then rebuild, replace the app, and check whether the permission must be granted again. Write the outcome in the README.
- [ ] **Step 4:** `git commit -am "release: 1.0.3 – indicatori sullo sfondo"`. Do not create the `v1.0.3` tag: the user publishes it.
