//! Riconoscimento automatico delle call: si guarda quali app stanno usando il microfono.
//! Nessun permesso richiesto e nessun accesso all'audio: solo "chi" lo sta usando.

use crate::config::{Config, Detection};
use crate::platform;
use std::time::{Duration, Instant};

/// Un'app che in questo momento usa il microfono.
#[derive(Clone, Debug, PartialEq)]
pub struct MicUser {
    /// Identificativo stabile: percorso dell'eseguibile (Windows) o bundle id (macOS).
    pub id: String,
    /// Nome leggibile, es. "Microsoft Teams" o "Google Meet (Chrome)".
    pub label: String,
    /// App di comunicazione riconosciuta.
    pub known_call_app: bool,
}

/// Dato grezzo fornito dal sistema operativo.
pub struct RawMicUser {
    pub id: String,
    /// Titoli delle finestre dell'app (solo Windows), per riconoscere Meet & co. nel browser.
    pub titles: Vec<String>,
}

enum Kind {
    Call,
    Browser,
    Never,
}

/// (parte dell'identificativo in minuscolo, nome, tipo)
const APPS: &[(&str, &str, Kind)] = &[
    ("teams", "Microsoft Teams", Kind::Call),
    ("slack", "Slack", Kind::Call),
    ("zoom", "Zoom", Kind::Call),
    ("webex", "Webex", Kind::Call),
    ("ciscocollabhost", "Webex", Kind::Call),
    ("cisco-systems.spark", "Webex", Kind::Call),
    ("discord", "Discord", Kind::Call),
    ("skype", "Skype", Kind::Call),
    ("whatsapp", "WhatsApp", Kind::Call),
    ("telegram", "Telegram", Kind::Call),
    ("signal", "Signal", Kind::Call),
    ("facetime", "FaceTime", Kind::Call),
    ("avconferenced", "FaceTime", Kind::Call),
    ("gotomeeting", "GoTo Meeting", Kind::Call),
    ("g2mcomm", "GoTo Meeting", Kind::Call),
    ("ringcentral", "RingCentral", Kind::Call),
    ("3cx", "3CX", Kind::Call),
    ("jitsi", "Jitsi Meet", Kind::Call),
    ("lync", "Skype for Business", Kind::Call),
    // Browser: Meet, Teams web, Jitsi, Whereby... risultano come il browser.
    ("msedge", "Edge", Kind::Browser),
    ("edgemac", "Edge", Kind::Browser),
    ("chrome", "Chrome", Kind::Browser),
    ("chromium", "Chromium", Kind::Browser),
    ("firefox", "Firefox", Kind::Browser),
    ("brave", "Brave", Kind::Browser),
    ("opera", "Opera", Kind::Browser),
    ("vivaldi", "Vivaldi", Kind::Browser),
    ("company.thebrowser", "Arc", Kind::Browser),
    ("arc.exe", "Arc", Kind::Browser),
    ("safari", "Safari", Kind::Browser),
    ("com.apple.webkit", "Safari", Kind::Browser),
    // Usi del microfono che non sono call.
    ("dictation", "Dettatura", Kind::Never),
    ("speechrecognition", "Dettatura", Kind::Never),
    ("corespeech", "Siri", Kind::Never),
    ("siri", "Siri", Kind::Never),
    ("voicerecorder", "Registratore", Kind::Never),
    ("soundrecorder", "Registratore", Kind::Never),
    ("voicememos", "Memo vocali", Kind::Never),
    ("devlogica-tools", "DevLogica Tools", Kind::Never),
];

/// Servizi di call riconoscibili dal titolo della finestra del browser.
const WEB_CALLS: &[(&str, &str)] = &[
    ("meet.google", "Google Meet"),
    ("google meet", "Google Meet"),
    ("meet -", "Google Meet"),
    ("meet –", "Google Meet"),
    ("microsoft teams", "Microsoft Teams"),
    ("teams.microsoft", "Microsoft Teams"),
    ("zoom", "Zoom"),
    ("jitsi", "Jitsi Meet"),
    ("whereby", "Whereby"),
    ("webex", "Webex"),
    ("slack", "Slack"),
    ("discord", "Discord"),
    ("huddle", "Slack"),
];

fn file_name(id: &str) -> String {
    id.rsplit(['\\', '/', '#']).next().unwrap_or(id).to_string()
}

pub fn classify(raw: &RawMicUser) -> MicUser {
    let low = raw.id.to_lowercase();
    for (key, name, kind) in APPS {
        if low.contains(key) {
            return match kind {
                Kind::Call => MicUser { id: raw.id.clone(), label: (*name).into(), known_call_app: true },
                Kind::Never => MicUser { id: raw.id.clone(), label: (*name).into(), known_call_app: false },
                Kind::Browser => {
                    let web = raw.titles.iter().find_map(|t| {
                        let t = t.to_lowercase();
                        WEB_CALLS.iter().find(|(k, _)| t.contains(k)).map(|(_, n)| *n)
                    });
                    let label = match web {
                        Some(service) => format!("{service} ({name})"),
                        None => format!("Browser ({name})"),
                    };
                    MicUser { id: raw.id.clone(), label, known_call_app: true }
                }
            };
        }
    }
    let label = file_name(&raw.id).trim_end_matches(".exe").to_string();
    MicUser { id: raw.id.clone(), label: if label.is_empty() { "App sconosciuta".into() } else { label }, known_call_app: false }
}

fn never(id: &str) -> bool {
    let low = id.to_lowercase();
    APPS.iter().any(|(k, _, kind)| matches!(kind, Kind::Never) && low.contains(k))
}

#[derive(Debug, PartialEq)]
pub enum Detect {
    /// Call iniziata, con il nome dell'app.
    Started(String),
    Stopped,
}

pub struct Detector {
    /// Chi usa il microfono adesso (anche app ignorate, per la diagnostica).
    pub users: Vec<MicUser>,
    pub in_call: Option<String>,
    candidate_since: Option<Instant>,
    idle_since: Option<Instant>,
}

/// Il microfono deve restare aperto almeno così prima di avvisare (evita note vocali e prove).
const START_AFTER: Duration = Duration::from_secs(4);
/// Breve chiusura del microfono (cambio dispositivo, mute di alcune app) non chiude la call.
const STOP_AFTER: Duration = Duration::from_secs(10);

impl Detector {
    pub fn new() -> Self {
        Self { users: Vec::new(), in_call: None, candidate_since: None, idle_since: None }
    }

    /// Da chiamare ogni secondo o due.
    pub fn poll(&mut self, cfg: &Config) -> Option<Detect> {
        self.users = platform::mic_users().iter().map(classify).collect();
        if cfg.detection == Detection::Off {
            self.candidate_since = None;
            return self.in_call.take().map(|_| Detect::Stopped);
        }
        let call = self
            .users
            .iter()
            .filter(|u| !cfg.ignored_apps.contains(&u.id) && !never(&u.id))
            .find(|u| u.known_call_app || cfg.detect_unknown)
            .map(|u| u.label.clone());
        let now = Instant::now();
        match (call, &self.in_call) {
            (Some(label), None) => {
                self.idle_since = None;
                let since = *self.candidate_since.get_or_insert(now);
                if now - since >= START_AFTER {
                    self.candidate_since = None;
                    self.in_call = Some(label.clone());
                    return Some(Detect::Started(label));
                }
            }
            (Some(_), Some(_)) => self.idle_since = None,
            (None, Some(_)) => {
                self.candidate_since = None;
                let since = *self.idle_since.get_or_insert(now);
                if now - since >= STOP_AFTER {
                    self.idle_since = None;
                    self.in_call = None;
                    return Some(Detect::Stopped);
                }
            }
            (None, None) => self.candidate_since = None,
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn raw(id: &str, titles: &[&str]) -> RawMicUser {
        RawMicUser { id: id.into(), titles: titles.iter().map(|s| s.to_string()).collect() }
    }
    #[test]
    fn riconosce_le_app() {
        assert_eq!(classify(&raw(r"C:#Users#ema#AppData#Local#slack#slack.exe", &[])).label, "Slack");
        assert_eq!(classify(&raw("MSTeams_8wekyb3d8bbwe", &[])).label, "Microsoft Teams");
        assert_eq!(classify(&raw("com.microsoft.teams2", &[])).label, "Microsoft Teams");
        assert_eq!(classify(&raw("us.zoom.xos", &[])).label, "Zoom");
        let meet = classify(&raw(r"C:#Program Files#Google#Chrome#Application#chrome.exe", &["Meet - abc-defg-hij - Google Chrome"]));
        assert_eq!(meet.label, "Google Meet (Chrome)");
        assert!(meet.known_call_app);
        assert_eq!(classify(&raw("com.google.Chrome", &[])).label, "Browser (Chrome)");
        let x = classify(&raw(r"C:#Tools#obs64.exe", &[]));
        assert!(!x.known_call_app);
        assert_eq!(x.label, "obs64");
        assert!(never("com.apple.SpeechRecognitionCore.speechrecognitiond"));
    }
}
