//! Windows: la finestra dello sfondo diventa figlia della finestra del desktop
//! (WorkerW o Progman), così sta dietro alle icone ma sopra all'immagine di sfondo.

use crate::config::log;
use std::sync::atomic::{AtomicIsize, Ordering};
use windows::core::{w, BOOL, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::Shell::{
    SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE, QUNS_RUNNING_D3D_FULL_SCREEN,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// Ultimo genitore usato: se Explorer si riavvia, sparisce e bisogna ricreare le finestre.
static PARENT: AtomicIsize = AtomicIsize::new(0);

struct Desktop {
    /// Finestra a cui agganciarsi.
    parent: HWND,
    /// Windows 11 24H2 e successivi: Progman contiene sia le icone sia WorkerW.
    raised: Option<(HWND, HWND)>, // (SHELLDLL_DefView, WorkerW)
}

unsafe extern "system" fn find_defview(top: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        if FindWindowExW(Some(top), None, w!("SHELLDLL_DefView"), PCWSTR::null()).is_ok() {
            // La WorkerW giusta è la finestra successiva a quella che contiene le icone.
            if let Ok(worker) = FindWindowExW(None, Some(top), w!("WorkerW"), PCWSTR::null()) {
                *(lparam.0 as *mut HWND) = worker;
            }
        }
    }
    BOOL(1)
}

fn find_desktop() -> Option<Desktop> {
    unsafe {
        let progman = FindWindowW(w!("Progman"), PCWSTR::null()).ok()?;
        // Messaggio non documentato che chiede a Explorer di creare la finestra WorkerW.
        SendMessageTimeoutW(progman, 0x052C, WPARAM(0xD), LPARAM(0x1), SMTO_NORMAL, 1000, None);
        SendMessageTimeoutW(progman, 0x052C, WPARAM(0), LPARAM(0), SMTO_NORMAL, 1000, None);

        // Windows 10 / 11 fino a 23H2: WorkerW di primo livello, dietro alle icone.
        let mut worker = HWND::default();
        let _ = EnumWindows(Some(find_defview), LPARAM(&mut worker as *mut HWND as isize));
        if !worker.is_invalid() {
            log("Desktop: modalità WorkerW classica");
            return Some(Desktop { parent: worker, raised: None });
        }

        // Windows 11 24H2+: icone (SHELLDLL_DefView) e WorkerW sono figlie di Progman.
        let defview = FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), PCWSTR::null()).ok();
        let worker = FindWindowExW(Some(progman), None, w!("WorkerW"), PCWSTR::null()).ok();
        if let (Some(defview), Some(worker)) = (defview, worker) {
            log("Desktop: modalità Progman (Windows 11 24H2+)");
            return Some(Desktop { parent: progman, raised: Some((defview, worker)) });
        }

        log("Desktop: WorkerW non trovata, uso Progman");
        Some(Desktop { parent: progman, raised: None })
    }
}

fn hwnd_of(window: &Window) -> Option<HWND> {
    match window.window_handle().ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as *mut _)),
        _ => None,
    }
}

pub fn attach(window: &Window, pos: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> bool {
    let Some(hwnd) = hwnd_of(window) else { return false };
    let Some(desk) = find_desktop() else {
        log("Impossibile trovare la finestra del desktop");
        return false;
    };
    unsafe {
        // Da finestra "popup" a finestra figlia, senza bordi.
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
        let style = (style & !(WS_POPUP.0 | WS_CAPTION.0 | WS_THICKFRAME.0 | WS_OVERLAPPEDWINDOW.0)) | WS_CHILD.0;
        SetWindowLongPtrW(hwnd, GWL_STYLE, style as isize);
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, (ex & !WS_EX_APPWINDOW.0) as isize);

        if SetParent(hwnd, Some(desk.parent)).is_err() {
            log("SetParent non riuscito");
            return false;
        }
        PARENT.store(desk.parent.0 as isize, Ordering::Relaxed);

        // Coordinate dello schermo → coordinate interne alla finestra genitore.
        let mut pt = POINT { x: pos.x, y: pos.y };
        let _ = ScreenToClient(desk.parent, &mut pt);
        let flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
        match desk.raised {
            Some((defview, worker)) => {
                // Subito sotto alle icone…
                let _ = SetWindowPos(hwnd, Some(defview), pt.x, pt.y, size.width as i32, size.height as i32, flags);
                // …e sopra alla WorkerW che contiene l'immagine di sfondo statica.
                let _ = SetWindowPos(worker, Some(hwnd), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
            None => {
                let _ = SetWindowPos(hwnd, None, pt.x, pt.y, size.width as i32, size.height as i32, flags | SWP_NOZORDER);
            }
        }
    }
    true
}

/// Vero se la finestra del desktop a cui eravamo agganciati non esiste più (es. Explorer riavviato).
pub fn desktop_lost() -> bool {
    let p = PARENT.load(Ordering::Relaxed);
    p != 0 && unsafe { !IsWindow(Some(HWND(p as *mut _))).as_bool() }
}

/// Vero se c'è un gioco, una presentazione o un'app a schermo intero.
pub fn fullscreen_busy() -> bool {
    match unsafe { SHQueryUserNotificationState() } {
        Ok(s) => s == QUNS_BUSY || s == QUNS_RUNNING_D3D_FULL_SCREEN || s == QUNS_PRESENTATION_MODE,
        Err(_) => false,
    }
}

/// All'uscita ri-applica lo sfondo di Windows, altrimenti resterebbe l'ultimo fotogramma.
pub fn restore_desktop() {
    unsafe {
        let mut buf = [0u16; 1024];
        if SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr().cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)).is_ok() {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(buf.as_mut_ptr().cast()), SPIF_SENDCHANGE);
        }
    }
}

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_NAME: &str = "DevLogicaTools";

pub fn autostart_enabled() -> bool {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    hkcu.open_subkey(RUN_KEY).and_then(|k| k.get_value::<String, _>(RUN_NAME)).is_ok()
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(RUN_KEY).map_err(|e| e.to_string())?;
    if on {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        key.set_value(RUN_NAME, &format!("\"{}\"", exe.display())).map_err(|e| e.to_string())
    } else {
        match key.delete_value(RUN_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Microfono: chi lo sta usando adesso.
// È la stessa informazione dell'icona del microfono nella barra di sistema: per ogni app
// il registro conserva inizio e fine dell'ultimo utilizzo; fine = 0 significa "in uso ora".
// ---------------------------------------------------------------------------

use crate::detect::RawMicUser;
use std::collections::HashMap;
use windows::core::PWSTR;
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};

const MIC_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\CapabilityAccessManager\ConsentStore\microphone";

fn in_use(k: &winreg::RegKey) -> bool {
    let start: u64 = k.get_value("LastUsedTimeStart").unwrap_or(0);
    let stop: u64 = k.get_value("LastUsedTimeStop").unwrap_or(1);
    start > 0 && stop == 0
}

pub fn mic_users() -> Vec<RawMicUser> {
    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    let Ok(root) = hkcu.open_subkey(MIC_KEY) else { return Vec::new() };
    let mut ids: Vec<String> = Vec::new();
    for name in root.enum_keys().flatten() {
        let Ok(key) = root.open_subkey(&name) else { continue };
        if name == "NonPackaged" {
            // App "classiche": la chiave è il percorso dell'eseguibile con # al posto di \
            for sub in key.enum_keys().flatten() {
                if key.open_subkey(&sub).map(|k| in_use(&k)).unwrap_or(false) {
                    ids.push(sub.replace('#', "\\"));
                }
            }
        } else if in_use(&key) {
            // App dello Store (es. MSTeams_8wekyb3d8bbwe)
            ids.push(name);
        }
    }
    if ids.is_empty() {
        return Vec::new();
    }
    let titles = window_titles_by_exe();
    ids.into_iter()
        .map(|id| {
            let t = titles.get(&id.to_lowercase()).cloned().unwrap_or_default();
            RawMicUser { id, titles: t }
        })
        .collect()
}

unsafe extern "system" fn collect_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    unsafe {
        if IsWindowVisible(hwnd).as_bool() {
            let mut buf = [0u16; 512];
            let n = GetWindowTextW(hwnd, &mut buf);
            if n > 0 {
                let mut pid = 0u32;
                GetWindowThreadProcessId(hwnd, Some(&mut pid));
                let list = &mut *(lparam.0 as *mut Vec<(u32, String)>);
                list.push((pid, String::from_utf16_lossy(&buf[..n as usize])));
            }
        }
    }
    BOOL(1)
}

fn exe_path(pid: u32) -> Option<String> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut size).is_ok();
        let _ = CloseHandle(h);
        ok.then(|| String::from_utf16_lossy(&buf[..size as usize]))
    }
}

/// Titoli delle finestre visibili, raggruppati per eseguibile (percorso in minuscolo).
/// Servono a capire se il browser che usa il microfono ha aperto Meet, Teams web, ecc.
fn window_titles_by_exe() -> HashMap<String, Vec<String>> {
    let mut list: Vec<(u32, String)> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(collect_window), LPARAM(&mut list as *mut _ as isize));
    }
    let mut paths: HashMap<u32, Option<String>> = HashMap::new();
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for (pid, title) in list {
        let path = paths.entry(pid).or_insert_with(|| exe_path(pid)).clone();
        if let Some(p) = path {
            out.entry(p.to_lowercase()).or_default().push(title);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Finestre di avviso e utilità
// ---------------------------------------------------------------------------

/// Su Windows bastano le opzioni di winit (sempre in primo piano, nessuna attivazione).
pub fn make_overlay(_window: &Window) {}

pub fn open_url(url: &str) {
    let _ = std::process::Command::new("rundll32").args(["url.dll,FileProtocolHandler", url]).spawn();
}
