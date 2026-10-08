//! macOS: la finestra viene portata al livello "desktop" (sotto le icone),
//! resa visibile in tutti gli Spaces e trasparente ai clic.

use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_foundation::NSRect;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGWindowLevelForKey(key: i32) -> i32;
}
const K_CG_DESKTOP_WINDOW_LEVEL_KEY: i32 = 2;

// NSWindowCollectionBehavior
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const STATIONARY: usize = 1 << 4;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_NONE: usize = 1 << 9;

pub fn attach(window: &Window, _pos: PhysicalPosition<i32>, _size: PhysicalSize<u32>) -> bool {
    let Ok(handle) = window.window_handle() else { return false };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return false };
    unsafe {
        let view: &AnyObject = h.ns_view.cast::<AnyObject>().as_ref();
        let ns_window: *mut AnyObject = msg_send![view, window];
        let Some(ns_window) = ns_window.as_ref() else { return false };
        let level = CGWindowLevelForKey(K_CG_DESKTOP_WINDOW_LEVEL_KEY) as isize;
        let _: () = msg_send![ns_window, setLevel: level];
        let _: () = msg_send![ns_window, setCollectionBehavior: CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_NONE];
        let _: () = msg_send![ns_window, setIgnoresMouseEvents: true];
        let _: () = msg_send![ns_window, setHasShadow: false];
        // Occupa tutto lo schermo su cui è stata creata, compresa la zona sotto la barra dei menu.
        let screen: *mut AnyObject = msg_send![ns_window, screen];
        if let Some(screen) = screen.as_ref() {
            let frame: NSRect = msg_send![screen, frame];
            let _: () = msg_send![ns_window, setFrame: frame, display: true];
        }
    }
    true
}

pub fn desktop_lost() -> bool {
    false
}

/// Su macOS le app a schermo intero stanno in uno Space separato: la finestra
/// risulta "coperta" (evento Occluded) e il rendering si ferma da solo.
pub fn fullscreen_busy() -> bool {
    false
}

pub fn restore_desktop() {}

fn agent_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join("Library/LaunchAgents/com.devlogica.tools.plist"))
}

pub fn autostart_enabled() -> bool {
    agent_path().map(|p| p.exists()).unwrap_or(false)
}

pub fn set_autostart(on: bool) -> Result<(), String> {
    let path = agent_path().ok_or("cartella utente non trovata")?;
    if !on {
        let _ = std::fs::remove_file(&path);
        return Ok(());
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>com.devlogica.tools</string>
  <key>ProgramArguments</key><array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict>
</plist>
"#,
        exe.display()
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, plist).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Microfono: CoreAudio dice se un dispositivo di ingresso è in uso da qualunque app
// e, da macOS 14, anche quali processi lo stanno usando. Non serve il permesso del
// microfono perché non si legge nessun audio.
// ---------------------------------------------------------------------------

use crate::detect::RawMicUser;
use std::ffi::{c_char, c_void};

#[repr(C)]
struct PropAddr {
    selector: u32,
    scope: u32,
    element: u32,
}

#[link(name = "CoreAudio", kind = "framework")]
unsafe extern "C" {
    fn AudioObjectGetPropertyDataSize(id: u32, addr: *const PropAddr, qsize: u32, qdata: *const c_void, size: *mut u32) -> i32;
    fn AudioObjectGetPropertyData(id: u32, addr: *const PropAddr, qsize: u32, qdata: *const c_void, size: *mut u32, data: *mut c_void) -> i32;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFStringGetCString(s: *const c_void, buf: *mut c_char, size: isize, encoding: u32) -> u8;
    fn CFRelease(p: *const c_void);
}

const fn fourcc(s: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*s)
}
const SYSTEM_OBJECT: u32 = 1;
const SCOPE_GLOBAL: u32 = fourcc(b"glob");
const SCOPE_INPUT: u32 = fourcc(b"inpt");
const HW_DEVICES: u32 = fourcc(b"dev#");
const DEV_STREAMS: u32 = fourcc(b"stm#");
const DEV_RUNNING_SOMEWHERE: u32 = fourcc(b"gone");
// macOS 14+: elenco dei processi audio e loro stato
const HW_PROCESSES: u32 = fourcc(b"prs#");
const PROC_RUNNING_INPUT: u32 = fourcc(b"piri");
const PROC_BUNDLE_ID: u32 = fourcc(b"pbid");
const PROC_PID: u32 = fourcc(b"ppid");
const UTF8: u32 = 0x0800_0100;

fn addr(selector: u32, scope: u32) -> PropAddr {
    PropAddr { selector, scope, element: 0 }
}

fn get_u32(id: u32, selector: u32, scope: u32) -> Option<u32> {
    let a = addr(selector, scope);
    let mut v = 0u32;
    let mut size = 4u32;
    let st = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut v as *mut u32 as *mut c_void) };
    (st == 0).then_some(v)
}

fn get_list(id: u32, selector: u32, scope: u32) -> Option<Vec<u32>> {
    let a = addr(selector, scope);
    let mut size = 0u32;
    if unsafe { AudioObjectGetPropertyDataSize(id, &a, 0, std::ptr::null(), &mut size) } != 0 {
        return None;
    }
    let mut v = vec![0u32; size as usize / 4];
    if v.is_empty() {
        return Some(v);
    }
    let st = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, v.as_mut_ptr() as *mut c_void) };
    (st == 0).then(|| {
        v.truncate(size as usize / 4);
        v
    })
}

fn get_string(id: u32, selector: u32) -> Option<String> {
    let a = addr(selector, SCOPE_GLOBAL);
    let mut s: *const c_void = std::ptr::null();
    let mut size = std::mem::size_of::<*const c_void>() as u32;
    let st = unsafe { AudioObjectGetPropertyData(id, &a, 0, std::ptr::null(), &mut size, &mut s as *mut *const c_void as *mut c_void) };
    if st != 0 || s.is_null() {
        return None;
    }
    let mut buf = [0 as c_char; 512];
    let ok = unsafe { CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8) } != 0;
    unsafe { CFRelease(s) };
    if !ok {
        return None;
    }
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8(bytes).ok().filter(|s| !s.is_empty())
}

/// Vero se almeno un microfono (interno, cuffie, esterno) è aperto da qualche app.
fn any_input_running() -> bool {
    get_list(SYSTEM_OBJECT, HW_DEVICES, SCOPE_GLOBAL)
        .unwrap_or_default()
        .into_iter()
        .filter(|&d| {
            let a = addr(DEV_STREAMS, SCOPE_INPUT);
            let mut size = 0u32;
            unsafe { AudioObjectGetPropertyDataSize(d, &a, 0, std::ptr::null(), &mut size) == 0 && size > 0 }
        })
        .any(|d| get_u32(d, DEV_RUNNING_SOMEWHERE, SCOPE_GLOBAL).unwrap_or(0) != 0)
}

pub fn mic_users() -> Vec<RawMicUser> {
    if !any_input_running() {
        return Vec::new();
    }
    // macOS 14+: quali processi stanno registrando
    if let Some(procs) = get_list(SYSTEM_OBJECT, HW_PROCESSES, SCOPE_GLOBAL) {
        let users: Vec<RawMicUser> = procs
            .into_iter()
            .filter(|&p| get_u32(p, PROC_RUNNING_INPUT, SCOPE_GLOBAL).unwrap_or(0) != 0)
            .map(|p| {
                let id = get_string(p, PROC_BUNDLE_ID)
                    .or_else(|| get_u32(p, PROC_PID, SCOPE_GLOBAL).map(|pid| format!("pid {pid}")))
                    .unwrap_or_else(|| "sconosciuta".into());
                RawMicUser { id, titles: Vec::new() }
            })
            .collect();
        if !users.is_empty() {
            return users;
        }
    }
    // macOS più vecchi: si sa solo che il microfono è in uso.
    vec![RawMicUser { id: "microfono".into(), titles: Vec::new() }]
}

// ---------------------------------------------------------------------------
// Finestre di avviso: sopra a tutto, anche alle app a schermo intero, in ogni Space.
// ---------------------------------------------------------------------------

const K_CG_SCREEN_SAVER_WINDOW_LEVEL_KEY: i32 = 13;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;

pub fn make_overlay(window: &Window) {
    let Ok(handle) = window.window_handle() else { return };
    let RawWindowHandle::AppKit(h) = handle.as_raw() else { return };
    unsafe {
        let view: &AnyObject = h.ns_view.cast::<AnyObject>().as_ref();
        let ns_window: *mut AnyObject = msg_send![view, window];
        let Some(ns_window) = ns_window.as_ref() else { return };
        let level = CGWindowLevelForKey(K_CG_SCREEN_SAVER_WINDOW_LEVEL_KEY) as isize;
        let _: () = msg_send![ns_window, setLevel: level];
        let _: () = msg_send![ns_window, setCollectionBehavior: CAN_JOIN_ALL_SPACES | FULL_SCREEN_AUXILIARY | STATIONARY | IGNORES_CYCLE];
        let _: () = msg_send![ns_window, setHidesOnDeactivate: false];
    }
}

pub fn open_url(url: &str) {
    let _ = std::process::Command::new("open").arg(url).spawn();
}
