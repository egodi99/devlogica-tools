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
            Some(Asset { url: a.get("browser_download_url")?.as_str()?.to_string(), size: a.get("size")?.as_u64()? })
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

/// Restituisce (versione, pagina della release) se ce n'è una più recente.
/// Sostituito dagli stati di aggiornamento (vedi `fetch`).
pub fn check() -> Option<(String, String)> {
    fetch().ok().flatten().map(|r| (r.tag, r.page))
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
                  {"name":"DevLogica-Tools-macOS.zip","size":456,"browser_download_url":"https://e/m.zip"}]}"#;

    #[test]
    fn scegli_asset_presente() {
        let r = parse_release(JSON, "DevLogica-Tools-macOS.zip").unwrap();
        assert_eq!(r.tag, "v1.0.5");
        assert_eq!(r.page, "https://github.com/x/y/releases/tag/v1.0.5");
        assert_eq!(r.asset, Some(Asset { url: "https://e/m.zip".into(), size: 456 }));
        assert_eq!(parse_release(JSON, "DevLogica-Tools-Windows.exe").unwrap().asset.unwrap().size, 123);
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
}
