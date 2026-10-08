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

/// Restituisce (versione, pagina della release) se ce n'è una più recente.
pub fn check() -> Option<(String, String)> {
    let repo = repo()?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let mut cmd = std::process::Command::new("curl");
    cmd.args(["-s", "-m", "15", "-H", "User-Agent: devlogica-tools", &url]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW: nessuna console che lampeggia
    }
    let out = cmd.output().ok()?;
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    let tag = json.get("tag_name")?.as_str()?.to_string();
    let page = json.get("html_url").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    if is_newer(&tag, env!("CARGO_PKG_VERSION")) {
        log(format!("Aggiornamento disponibile: {tag}"));
        Some((tag, page))
    } else {
        None
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
}
