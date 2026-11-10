//! Aggiornamento dell'app: scarica il file della release, lo verifica e sostituisce l'app
//! in uso, poi avvia la versione nuova. La copia in uso non viene mai sovrascritta: si
//! rinomina, e solo quando il file nuovo è completo e verificato.

use crate::config::{self, log};
use crate::update::Asset;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Downloading,
    Installing,
}

/// Su macOS: dall'eseguibile dentro `X.app/Contents/MacOS/` risale al bundle `X.app`.
pub fn bundle_from_exe(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let app = contents.parent()?;
    let ok = macos.file_name()? == "MacOS" && contents.file_name()? == "Contents" && app.extension()? == "app";
    ok.then(|| app.to_path_buf())
}

/// (copia vecchia, file nuovo), accanto a `target`.
pub fn swap_paths(target: &Path) -> (PathBuf, PathBuf) {
    let with = |suffix: &str| {
        let mut name = target.file_name().unwrap_or_default().to_os_string();
        name.push(suffix);
        target.with_file_name(name)
    };
    (with(".vecchia"), with(".nuova"))
}

pub fn size_ok(path: &Path, expected: u64) -> bool {
    std::fs::metadata(path).map(|m| m.len() == expected).unwrap_or(false)
}

/// Cosa va sostituito: il bundle .app su macOS, l'eseguibile su Windows.
fn target() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| format!("percorso dell'app sconosciuto: {e}"))?;
    if cfg!(target_os = "macos") {
        bundle_from_exe(&exe).ok_or_else(|| "l'app non è installata come .app".to_string())
    } else {
        Ok(exe)
    }
}

fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

fn run(c: &mut Command, what: &str) -> Result<String, String> {
    let out = c.output().map_err(|e| format!("{what}: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        log(format!("{what} non riuscito: {}", String::from_utf8_lossy(&out.stderr).trim()));
        Err(format!("{what} non riuscito"))
    }
}

fn remove_all(p: &Path) {
    if p.is_dir() {
        let _ = std::fs::remove_dir_all(p);
    } else if p.exists() {
        let _ = std::fs::remove_file(p);
    }
}

fn download(url: &str, dest: &Path, size: u64) -> Result<(), String> {
    run(command("curl").args(["-L", "-f", "-s", "-m", "600", "-H", "User-Agent: devlogica-tools", "-o"]).arg(dest).arg(url), "download")?;
    if !size_ok(dest, size) {
        return Err("il file scaricato è incompleto".into());
    }
    Ok(())
}

/// Prepara accanto a `target` la versione nuova, completa e verificata, in `new`.
#[cfg(target_os = "macos")]
fn prepare(asset: &Asset, tag: &str, new: &Path, progress: &impl Fn(Phase)) -> Result<(), String> {
    let work = config::app_dir().join("aggiornamento");
    remove_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| format!("cartella temporanea: {e}"))?;
    let zip = work.join("nuova.zip");
    let result = (|| {
        download(&asset.url, &zip, asset.size)?;
        progress(Phase::Installing);
        let out = work.join("estratta");
        run(command("ditto").args(["-x", "-k"]).arg(&zip).arg(&out), "estrazione")?;
        let app = out.join("DevLogica Tools.app");
        if !app.exists() {
            return Err("l'archivio non contiene l'app".to_string());
        }
        run(command("codesign").args(["--verify", "--deep"]).arg(&app), "verifica della firma")?;
        let plist = app.join("Contents/Info");
        let version = run(command("defaults").arg("read").arg(&plist).arg("CFBundleShortVersionString"), "lettura della versione")?;
        if version != tag.trim_start_matches('v') {
            return Err(format!("versione inattesa nell'archivio ({version})"));
        }
        let _ = run(command("xattr").args(["-dr", "com.apple.quarantine"]).arg(&app), "rimozione quarantena");
        run(command("mv").arg(&app).arg(new), "copia dell'app")?;
        Ok(())
    })();
    remove_all(&work);
    result
}

#[cfg(not(target_os = "macos"))]
fn prepare(asset: &Asset, _tag: &str, new: &Path, progress: &impl Fn(Phase)) -> Result<(), String> {
    download(&asset.url, new, asset.size)?;
    progress(Phase::Installing);
    Ok(())
}

fn relaunch(target: &Path) -> std::io::Result<()> {
    if cfg!(target_os = "macos") {
        // `open` (senza -W) torna appena LaunchServices ha avviato l'app: se ne aspetta l'esito,
        // così un errore riporta alla versione in uso invece di lasciare l'utente senza app.
        let st = Command::new("open").arg("-n").arg(target).args(["--args", "--dopo-aggiornamento"]).status()?;
        if st.success() { Ok(()) } else { Err(std::io::Error::other(format!("open: {st}"))) }
    } else {
        let mut c = command(target);
        c.arg("--dopo-aggiornamento");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000 | 0x0000_0008); // CREATE_NO_WINDOW | DETACHED_PROCESS
        }
        c.spawn().map(|_| ())
    }
}

/// Mette `new` al posto di `target` e avvia la versione nuova con `relaunch`. La copia in uso
/// si rinomina in "<nome>.vecchia" (consentito anche mentre è aperta). Se qualcosa non va,
/// tutto torna com'era e `new` viene cancellato.
pub fn swap(target: &Path, new: &Path, relaunch: impl FnOnce(&Path) -> std::io::Result<()>) -> Result<(), String> {
    let (old, _) = swap_paths(target);
    remove_all(&old);
    if let Err(e) = std::fs::rename(target, &old) {
        remove_all(new);
        log(format!("Rinomina della versione in uso non riuscita: {e}"));
        return Err("la cartella dell'app non è scrivibile".into());
    }
    if let Err(e) = std::fs::rename(new, target) {
        let _ = std::fs::rename(&old, target);
        remove_all(new);
        log(format!("Scambio non riuscito: {e}"));
        return Err("sostituzione non riuscita".into());
    }
    if let Err(e) = relaunch(target) {
        let _ = std::fs::rename(target, new);
        let _ = std::fs::rename(&old, target);
        remove_all(new);
        log(format!("Avvio della versione nuova non riuscito: {e}"));
        return Err("avvio della versione nuova non riuscito".into());
    }
    Ok(())
}

/// Scarica, verifica, sostituisce e avvia la versione nuova. `Ok` = l'app in uso deve chiudersi.
/// In caso di errore l'app in uso resta esattamente com'era.
pub fn install(asset: &Asset, tag: &str, progress: impl Fn(Phase)) -> Result<(), String> {
    let target = target()?;
    let (old, new) = swap_paths(&target);
    remove_all(&old);
    remove_all(&new);
    log(format!("Aggiornamento a {tag}: {}", target.display()));
    progress(Phase::Downloading);
    if let Err(e) = prepare(asset, tag, &new, &progress) {
        remove_all(&new);
        log(format!("Aggiornamento non riuscito: {e}"));
        return Err(e);
    }
    if let Err(e) = swap(&target, &new, relaunch) {
        log(format!("Aggiornamento non riuscito: {e}"));
        return Err(e);
    }
    log(format!("Versione {tag} installata, riavvio"));
    Ok(())
}

/// Cancella i resti di un aggiornamento (copia vecchia, file parziali). Si chiama all'avvio.
pub fn cleanup() {
    if let Ok(t) = target() {
        let (old, new) = swap_paths(&t);
        remove_all(&old);
        remove_all(&new);
    }
    if cfg!(target_os = "macos") {
        remove_all(&config::app_dir().join("aggiornamento"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundle_da_exe_app() {
        let exe = Path::new("/Applications/DevLogica Tools.app/Contents/MacOS/devlogica-tools");
        assert_eq!(bundle_from_exe(exe), Some(PathBuf::from("/Applications/DevLogica Tools.app")));
    }

    #[test]
    fn bundle_da_exe_cargo() {
        assert_eq!(bundle_from_exe(Path::new("target/release/devlogica-tools")), None);
        assert_eq!(bundle_from_exe(Path::new("/x/Contents/MacOS/bin")), None);
    }

    #[test]
    fn percorsi_scambio() {
        assert_eq!(swap_paths(Path::new("/x/App.app")), (PathBuf::from("/x/App.app.vecchia"), PathBuf::from("/x/App.app.nuova")));
        let (old, new) = swap_paths(Path::new("C:/t/DT.exe"));
        assert_eq!(old.file_name().unwrap(), "DT.exe.vecchia");
        assert_eq!(new.file_name().unwrap(), "DT.exe.nuova");
    }

    #[test]
    fn verifica_dimensione() {
        let dir = std::env::temp_dir().join(format!("dlt-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("f.bin");
        std::fs::write(&p, [0u8; 10]).unwrap();
        assert!(size_ok(&p, 10));
        assert!(!size_ok(&p, 11));
        assert!(!size_ok(&dir.join("manca"), 10));
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("dlt-swap-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn scambio_riuscito() {
        let d = dir("ok");
        let target = d.join("DT.exe");
        let (old, new) = swap_paths(&target);
        std::fs::write(&target, "vecchia").unwrap();
        std::fs::write(&new, "nuova").unwrap();
        let mut launched = None;
        assert_eq!(swap(&target, &new, |p| { launched = Some(p.to_path_buf()); Ok(()) }), Ok(()));
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "nuova");
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "vecchia");
        assert_eq!(launched, Some(target.clone()));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn scambio_riavvio_fallito_ripristina() {
        let d = dir("ko");
        let target = d.join("DT.exe");
        let (old, new) = swap_paths(&target);
        std::fs::write(&target, "vecchia").unwrap();
        std::fs::write(&new, "nuova").unwrap();
        let r = swap(&target, &new, |_| Err(std::io::Error::other("open fallito")));
        assert!(r.is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "vecchia");
        assert!(!old.exists());
        assert!(!new.exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn scambio_con_vecchia_rimasta() {
        let d = dir("resto");
        let target = d.join("App.app");
        let (old, new) = swap_paths(&target);
        std::fs::create_dir_all(target.join("Contents")).unwrap();
        std::fs::create_dir_all(new.join("Contents")).unwrap();
        std::fs::write(new.join("Contents/v"), "nuova").unwrap();
        std::fs::create_dir_all(&old).unwrap(); // resto di un aggiornamento precedente
        assert_eq!(swap(&target, &new, |_| Ok(())), Ok(()));
        assert_eq!(std::fs::read_to_string(target.join("Contents/v")).unwrap(), "nuova");
        let _ = std::fs::remove_dir_all(&d);
    }
}
