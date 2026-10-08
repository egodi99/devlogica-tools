//! Configurazione persistente (JSON nella cartella di configurazione dell'utente) e log.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

pub const APP_NAME: &str = "DevLogica Tools";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Lo stesso sfondo, ripetuto su ogni schermo.
    Same,
    /// Uno sfondo diverso per ogni schermo.
    PerMonitor,
    /// Un unico sfondo esteso su tutti gli schermi.
    Span,
}

/// Risoluzione a cui si disegna lo sfondo. Meno pixel = meno memoria per i buffer video.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quality {
    /// Al massimo 1440 righe: nessuna differenza fino al 1440p, molta meno memoria sui 4K.
    Auto,
    /// Risoluzione nativa dello schermo.
    Full,
    /// Metà risoluzione: il consumo minimo possibile.
    Low,
}

impl Quality {
    pub fn scale(self, height: u32) -> f32 {
        match self {
            Quality::Full => 1.0,
            Quality::Low => 0.5,
            Quality::Auto => (1440.0 / height.max(1) as f32).min(1.0),
        }
    }
}

/// Come comportarsi quando il microfono viene usato da un'app di call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Detection {
    /// Avvisa l'ufficio da solo.
    Auto,
    /// Chiede conferma con un piccolo riquadro.
    Ask,
    /// Solo manuale, dal menu.
    Off,
}

/// Quanto è vistoso l'avviso quando un collega è in call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertStyle {
    /// Bordo rosso pulsante attorno agli schermi + riquadro con l'elenco (come Call Alert).
    Full,
    /// Solo il riquadro con l'elenco.
    Panel,
    /// Solo l'icona nella barra di sistema diventa rossa.
    Tray,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // ---- persona ----
    pub name: String,
    pub volume: f32,
    // ---- call ----
    pub detection: Detection,
    /// Considera call anche le app sconosciute che usano il microfono.
    pub detect_unknown: bool,
    /// App da non considerare mai (identificativo: percorso dell'eseguibile o bundle id).
    pub ignored_apps: Vec<String>,
    pub alert_style: AlertStyle,
    // ---- sfondi ----
    pub wallpaper_enabled: bool,
    pub mode: Mode,
    /// Sfondo usato nelle modalità `Same` e `Span` (e come predefinito per gli schermi nuovi).
    pub wallpaper: String,
    /// Sfondo per schermo, indicizzato dalla chiave dello schermo (nome + risoluzione).
    pub per_monitor: HashMap<String, String>,
    pub fps: u32,
    pub quality: Quality,
    pub show_logo: bool,
    pub paused: bool,
    /// Mette in pausa gli sfondi quando un'applicazione è a schermo intero (solo Windows).
    pub pause_on_fullscreen: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            name: String::new(),
            volume: 0.7,
            detection: Detection::Auto,
            detect_unknown: false,
            ignored_apps: Vec::new(),
            alert_style: AlertStyle::Full,
            wallpaper_enabled: true,
            mode: Mode::Same,
            wallpaper: "lame".into(),
            per_monitor: HashMap::new(),
            fps: 30,
            quality: Quality::Auto,
            show_logo: true,
            paused: false,
            pause_on_fullscreen: true,
        }
    }
}

pub fn app_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    base.join(APP_NAME)
}

/// Cartella della versione precedente (solo sfondi), da cui importare le impostazioni.
fn legacy_dir() -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(std::env::temp_dir);
    base.join("DevLogica Wallpaper")
}

pub fn custom_wallpapers_dir() -> PathBuf {
    app_dir().join("sfondi")
}

fn config_path() -> PathBuf {
    app_dir().join("config.json")
}

impl Config {
    /// Carica la configurazione. Al primo avvio importa quella di DevLogica Wallpaper,
    /// compresi gli sfondi personalizzati.
    pub fn load() -> Self {
        if !config_path().exists() {
            let old = legacy_dir();
            if old.join("config.json").exists() {
                let _ = std::fs::create_dir_all(custom_wallpapers_dir());
                let _ = std::fs::copy(old.join("config.json"), config_path());
                if let Ok(entries) = std::fs::read_dir(old.join("sfondi")) {
                    for e in entries.flatten() {
                        let _ = std::fs::copy(e.path(), custom_wallpapers_dir().join(e.file_name()));
                    }
                }
                log("Impostazioni importate da DevLogica Wallpaper");
            }
        }
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(app_dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(config_path(), s);
        }
    }

    pub fn fps(&self) -> u32 {
        self.fps.clamp(5, 60)
    }

    /// Nome mostrato ai colleghi (mai vuoto).
    pub fn display_name(&self) -> String {
        let n = self.name.trim();
        if n.is_empty() { "Utente".into() } else { n.to_string() }
    }
}

/// Piccolo log su file, utile per capire cosa succede su un PC dell'ufficio.
pub fn log(msg: impl AsRef<str>) {
    use std::io::Write;
    let _ = std::fs::create_dir_all(app_dir());
    let path = app_dir().join("log.txt");
    // Evita che il file cresca all'infinito.
    if std::fs::metadata(&path).map(|m| m.len() > 512 * 1024).unwrap_or(false) {
        let _ = std::fs::remove_file(&path);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let _ = writeln!(f, "[{secs}] {}", msg.as_ref());
    }
    eprintln!("{}", msg.as_ref());
}

/// Millisecondi dall'epoca Unix, come `Date.now()` di JavaScript (compatibilità con Call Alert).
pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}
