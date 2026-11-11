//! Controllo aggiornamenti dalle release di GitHub. Usa `curl`, presente sia su macOS sia su
//! Windows 10/11, così non serve una libreria HTTPS dentro l'app.

use crate::config::log;

/// Repository da cui scaricare le release. Nelle build fatte da GitHub Actions è quello
/// della pipeline; in locale si può impostare con la variabile DEVLOGICA_REPO.
pub fn repo() -> Option<&'static str> {
    option_env!("DEVLOGICA_REPO").or(option_env!("GITHUB_REPOSITORY")).filter(|r| r.contains('/'))
}

fn parse(v: &str) -> Vec<u32> {
    v.trim_start_matches('v').split('.').map(|p| p.trim().parse().unwrap_or(0)).collect()
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    parse(latest) > parse(current)
}

/// File della release per questo sistema (vedi `.github/workflows/release.yml`).
#[cfg(windows)]
pub const ASSET_NAME: &str = "DevLogica-Tools-Windows.exe";
#[cfg(not(windows))]
pub const ASSET_NAME: &str = "DevLogica-Tools-macOS.zip";

#[derive(Clone, Debug, PartialEq)]
pub struct Asset {
    pub url: String,
    pub size: u64,
    /// SHA-256 calcolato da GitHub al caricamento (campo `digest`), in esadecimale minuscolo.
    pub sha256: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub tag: String,
    pub page: String,
    /// `None` se nella release manca il file per questo sistema.
    pub asset: Option<Asset>,
}

/// Versione in uso. DEVLOGICA_FAKE_VERSION (solo per provare gli aggiornamenti) la sostituisce.
pub fn current_version() -> String {
    std::env::var("DEVLOGICA_FAKE_VERSION").ok().filter(|v| !v.trim().is_empty()).unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string())
}

/// Legge la risposta di `releases/latest`. `None` se non è una release valida.
pub fn parse_release(json: &str, asset_name: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let tag = v.get("tag_name")?.as_str()?.to_string();
    let page = v.get("html_url").and_then(|x| x.as_str()).unwrap_or_default().to_string();
    let asset = v.get("assets").and_then(|a| a.as_array()).and_then(|list| {
        list.iter().find(|a| a.get("name").and_then(|n| n.as_str()) == Some(asset_name)).and_then(|a| {
            let sha256 = a.get("digest").and_then(|d| d.as_str()).and_then(|d| d.strip_prefix("sha256:")).map(|h| h.to_ascii_lowercase());
            Some(Asset { url: a.get("browser_download_url")?.as_str()?.to_string(), size: a.get("size")?.as_u64()?, sha256 })
        })
    });
    Some(Release { tag, page, asset })
}

/// Il controllo automatico avvisa una volta sola per versione; quello manuale sempre.
pub fn should_notify(manual: bool, tag: &str, notified: &str) -> bool {
    manual || tag != notified
}

/// Chiede a GitHub l'ultima release. `Ok(None)` = si ha già l'ultima versione.
pub fn fetch() -> Result<Option<Release>, String> {
    let repo = repo().ok_or("repository non configurato")?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-s", "-m", "15", "-H", "User-Agent: devlogica-tools", &url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: nessuna console che lampeggia
    }
    let out = cmd.output().map_err(|e| format!("curl non disponibile: {e}"))?;
    if !out.status.success() {
        return Err("nessuna connessione a GitHub".into());
    }
    let rel = parse_release(&String::from_utf8_lossy(&out.stdout), ASSET_NAME).ok_or("risposta di GitHub non valida")?;
    if is_newer(&rel.tag, &current_version()) {
        log(format!("Aggiornamento disponibile: {}", rel.tag));
        Ok(Some(rel))
    } else {
        Ok(None)
    }
}

/// Comandi offerti nel menu e nelle Impostazioni.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UpdateCmd {
    Check,
    Install,
    OpenPage,
}

/// Dove si trova l'aggiornamento dell'app.
#[derive(Clone, Debug, PartialEq)]
pub enum UpdateState {
    Idle,
    Checking { manual: bool },
    Available(Release),
    Downloading(Release),
    Installing(Release),
    Failed { release: Release, reason: String },
}

impl UpdateState {
    /// Voci da mostrare: (testo, attiva, comando).
    pub fn menu_items(&self) -> Vec<(String, bool, Option<UpdateCmd>)> {
        let check = ("Cerca aggiornamenti".to_string(), true, Some(UpdateCmd::Check));
        let page = ("Apri la pagina della release".to_string(), true, Some(UpdateCmd::OpenPage));
        let install = |r: &Release| (format!("Aggiorna alla {}", r.tag), true, Some(UpdateCmd::Install));
        match self {
            UpdateState::Idle => vec![check],
            UpdateState::Checking { .. } => vec![("Controllo in corso…".into(), false, None)],
            UpdateState::Available(r) if r.asset.is_some() => vec![install(r), check],
            UpdateState::Available(_) => vec![page, check],
            UpdateState::Downloading(_) => vec![("Download in corso…".into(), false, None), ("Cerca aggiornamenti".into(), false, None)],
            UpdateState::Installing(_) => vec![("Installazione…".into(), false, None), ("Cerca aggiornamenti".into(), false, None)],
            UpdateState::Failed { release, .. } if release.asset.is_some() => vec![install(release), page, check],
            UpdateState::Failed { .. } => vec![page, check],
        }
    }

    /// Vero se si può avviare un aggiornamento adesso (mai due insieme).
    pub fn can_start(&self) -> bool {
        match self {
            UpdateState::Available(r) | UpdateState::Failed { release: r, .. } => r.asset.is_some(),
            _ => false,
        }
    }

    pub fn release(&self) -> Option<&Release> {
        match self {
            UpdateState::Available(r) | UpdateState::Downloading(r) | UpdateState::Installing(r) | UpdateState::Failed { release: r, .. } => Some(r),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn versioni() {
        assert!(super::is_newer("v1.2.0", "1.1.9"));
        assert!(super::is_newer("1.10.0", "1.9.3"));
        assert!(!super::is_newer("v1.0.0", "1.0.0"));
    }

    use super::*;

    const JSON: &str = r#"{"tag_name":"v1.0.5","html_url":"https://github.com/x/y/releases/tag/v1.0.5",
        "assets":[{"name":"DevLogica-Tools-Windows.exe","size":123,"browser_download_url":"https://e/w.exe"},
                  {"name":"DevLogica-Tools-macOS.zip","size":456,"browser_download_url":"https://e/m.zip",
                   "digest":"sha256:2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824"}]}"#;

    #[test]
    fn scegli_asset_presente() {
        let r = parse_release(JSON, "DevLogica-Tools-macOS.zip").unwrap();
        assert_eq!(r.tag, "v1.0.5");
        assert_eq!(r.page, "https://github.com/x/y/releases/tag/v1.0.5");
        assert_eq!(r.asset, Some(Asset { url: "https://e/m.zip".into(), size: 456, sha256: Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824".into()) }));
        assert_eq!(parse_release(JSON, "DevLogica-Tools-Windows.exe").unwrap().asset.unwrap().size, 123);
    }

    #[test]
    fn hash_mancante_o_di_altro_tipo() {
        let w = parse_release(JSON, "DevLogica-Tools-Windows.exe").unwrap().asset.unwrap();
        assert_eq!(w.sha256, None);
        let md5 = r#"{"tag_name":"v1","assets":[{"name":"a","size":1,"browser_download_url":"u","digest":"md5:abc"}]}"#;
        assert_eq!(parse_release(md5, "a").unwrap().asset.unwrap().sha256, None);
    }

    #[test]
    fn scegli_asset_mancante() {
        let r = parse_release(JSON, "altro.zip").unwrap();
        assert_eq!(r.tag, "v1.0.5");
        assert_eq!(r.asset, None);
    }

    #[test]
    fn json_non_valido() {
        assert_eq!(parse_release("{", ASSET_NAME), None);
        assert_eq!(parse_release(r#"{"message":"Not Found"}"#, ASSET_NAME), None);
    }

    #[test]
    fn notifica_una_volta() {
        assert!(!should_notify(false, "v1.0.5", "v1.0.5"));
        assert!(should_notify(false, "v1.0.5", "v1.0.4"));
        assert!(should_notify(true, "v1.0.5", "v1.0.5"));
    }

    fn rel(asset: bool) -> Release {
        Release { tag: "v1.0.5".into(), page: "p".into(), asset: asset.then(|| Asset { url: "u".into(), size: 1, sha256: None }) }
    }
    fn texts(s: &UpdateState) -> Vec<(String, bool, Option<UpdateCmd>)> {
        s.menu_items()
    }
    use UpdateCmd::*;

    #[test]
    fn menu_idle() {
        assert_eq!(texts(&UpdateState::Idle), vec![("Cerca aggiornamenti".into(), true, Some(Check))]);
    }

    #[test]
    fn menu_checking() {
        assert_eq!(texts(&UpdateState::Checking { manual: true }), vec![("Controllo in corso…".into(), false, None)]);
    }

    #[test]
    fn menu_available() {
        assert_eq!(
            texts(&UpdateState::Available(rel(true))),
            vec![("Aggiorna alla v1.0.5".into(), true, Some(Install)), ("Cerca aggiornamenti".into(), true, Some(Check))]
        );
    }

    #[test]
    fn menu_downloading() {
        assert_eq!(
            texts(&UpdateState::Downloading(rel(true))),
            vec![("Download in corso…".into(), false, None), ("Cerca aggiornamenti".into(), false, None)]
        );
        assert_eq!(texts(&UpdateState::Installing(rel(true)))[0], ("Installazione…".into(), false, None));
    }

    #[test]
    fn menu_failed() {
        let f = UpdateState::Failed { release: rel(true), reason: "x".into() };
        assert_eq!(
            texts(&f),
            vec![
                ("Aggiorna alla v1.0.5".into(), true, Some(Install)),
                ("Apri la pagina della release".into(), true, Some(OpenPage)),
                ("Cerca aggiornamenti".into(), true, Some(Check)),
            ]
        );
    }

    #[test]
    fn menu_available_senza_asset() {
        assert_eq!(
            texts(&UpdateState::Available(rel(false))),
            vec![("Apri la pagina della release".into(), true, Some(OpenPage)), ("Cerca aggiornamenti".into(), true, Some(Check))]
        );
    }

    #[test]
    fn stato_non_riparte() {
        assert!(UpdateState::Available(rel(true)).can_start());
        assert!(UpdateState::Failed { release: rel(true), reason: "x".into() }.can_start());
        assert!(!UpdateState::Available(rel(false)).can_start());
        assert!(!UpdateState::Downloading(rel(true)).can_start());
        assert!(!UpdateState::Installing(rel(true)).can_start());
        assert!(!UpdateState::Idle.can_start());
    }
}
