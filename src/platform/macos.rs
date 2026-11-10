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

// ---------------------------------------------------------------------------
// Statistiche per gli indicatori: CPU, RAM (Mach / sysctl) e ora locale.
// ---------------------------------------------------------------------------

#[repr(C)]
#[derive(Default)]
struct VmStatistics64 {
    free_count: u32,
    active_count: u32,
    inactive_count: u32,
    wire_count: u32,
    zero_fill_count: u64,
    reactivations: u64,
    pageins: u64,
    pageouts: u64,
    faults: u64,
    cow_faults: u64,
    lookups: u64,
    hits: u64,
    purges: u64,
    purgeable_count: u32,
    speculative_count: u32,
    decompressions: u64,
    compressions: u64,
    swapins: u64,
    swapouts: u64,
    compressor_page_count: u32,
    throttled_count: u32,
    external_page_count: u32,
    internal_page_count: u32,
    total_uncompressed_pages_in_compressor: u64,
}

#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    tm_gmtoff: i64,
    tm_zone: *const c_char,
}

unsafe extern "C" {
    fn mach_host_self() -> u32;
    fn host_statistics(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn host_statistics64(host: u32, flavor: i32, info: *mut i32, count: *mut u32) -> i32;
    fn host_page_size(host: u32, size: *mut usize) -> i32;
    fn sysctlbyname(name: *const c_char, old: *mut c_void, oldlen: *mut usize, new: *const c_void, newlen: usize) -> i32;
    fn time(t: *mut i64) -> i64;
    fn localtime_r(t: *const i64, out: *mut Tm) -> *mut Tm;
}

const HOST_CPU_LOAD_INFO: i32 = 3;
const HOST_VM_INFO64: i32 = 4;

/// Tick complessivi della CPU: [user, system, idle, nice].
pub fn cpu_ticks() -> Option<[u64; 4]> {
    let mut t = [0u32; 4];
    let mut count = 4u32;
    let st = unsafe { host_statistics(mach_host_self(), HOST_CPU_LOAD_INFO, t.as_mut_ptr() as *mut i32, &mut count) };
    (st == 0).then(|| t.map(|v| v as u64))
}

/// (internal, purgeable, wired, compressor, dimensione pagina)
pub fn vm_pages() -> Option<(u64, u64, u64, u64, u64)> {
    let mut vm = VmStatistics64::default();
    let mut count = (std::mem::size_of::<VmStatistics64>() / 4) as u32;
    let host = unsafe { mach_host_self() };
    let st = unsafe { host_statistics64(host, HOST_VM_INFO64, &mut vm as *mut _ as *mut i32, &mut count) };
    let mut page = 0usize;
    if st != 0 || unsafe { host_page_size(host, &mut page) } != 0 {
        return None;
    }
    Some((vm.internal_page_count as u64, vm.purgeable_count as u64, vm.wire_count as u64, vm.compressor_page_count as u64, page as u64))
}

pub fn mem_total() -> Option<u64> {
    let mut v = 0u64;
    let mut len = std::mem::size_of::<u64>();
    let st = unsafe { sysctlbyname(c"hw.memsize".as_ptr(), &mut v as *mut u64 as *mut c_void, &mut len, std::ptr::null(), 0) };
    (st == 0).then_some(v)
}

/// Ora e minuti locali.
pub fn local_hm() -> (u32, u32) {
    unsafe {
        let now = time(std::ptr::null_mut());
        let mut tm: Tm = std::mem::zeroed();
        if localtime_r(&now, &mut tm).is_null() {
            return (0, 0);
        }
        (tm.tm_hour as u32, tm.tm_min as u32)
    }
}

// ---------------------------------------------------------------------------
// Posizione del Dock per gli indicatori: esatta con l'Accessibilità, altrimenti
// si capisce almeno quale schermo lo ha confrontando `frame` e `visibleFrame`.
// ---------------------------------------------------------------------------

use crate::indicators::{DockInfo, GRect};

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct CgPoint {
    x: f64,
    y: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    static kAXTrustedCheckOptionPrompt: *const c_void;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
    fn AXUIElementCopyAttributeValue(element: *const c_void, attribute: *const c_void, value: *mut *const c_void) -> i32;
    fn AXValueGetValue(value: *const c_void, kind: u32, out: *mut c_void) -> u8;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: u8;
    static kCFTypeDictionaryValueCallBacks: u8;
    fn CFDictionaryCreate(alloc: *const c_void, keys: *const *const c_void, values: *const *const c_void, n: isize, kcb: *const c_void, vcb: *const c_void) -> *const c_void;
    fn CFStringCreateWithCString(alloc: *const c_void, s: *const c_char, encoding: u32) -> *const c_void;
    fn CFArrayGetCount(a: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(a: *const c_void, i: isize) -> *const c_void;
}

const AX_VALUE_CGPOINT: u32 = 1;
const AX_VALUE_CGSIZE: u32 = 2;

/// Vero se l'app ha il permesso Accessibilità. Con `prompt` macOS mostra la sua richiesta.
pub fn ax_trusted(prompt: bool) -> bool {
    unsafe {
        if !prompt {
            return AXIsProcessTrustedWithOptions(std::ptr::null()) != 0;
        }
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let dict = CFDictionaryCreate(
            std::ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const u8 as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const u8 as *const c_void,
        );
        let ok = AXIsProcessTrustedWithOptions(dict) != 0;
        if !dict.is_null() {
            CFRelease(dict);
        }
        ok
    }
}

pub fn open_accessibility_settings() {
    open_url("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility");
}

/// Stringa CoreFoundation da rilasciare con CFRelease.
fn cfstr(s: &std::ffi::CStr) -> *const c_void {
    unsafe { CFStringCreateWithCString(std::ptr::null(), s.as_ptr(), UTF8) }
}

/// Valore di un attributo AX (da rilasciare con CFRelease), o null.
fn ax_attr(el: *const c_void, name: &std::ffi::CStr) -> *const c_void {
    let attr = cfstr(name);
    let mut out: *const c_void = std::ptr::null();
    let st = unsafe { AXUIElementCopyAttributeValue(el, attr, &mut out) };
    unsafe { CFRelease(attr) };
    if st == 0 { out } else { std::ptr::null() }
}

fn cf_to_string(s: *const c_void) -> Option<String> {
    let mut buf = [0 as c_char; 128];
    let ok = unsafe { CFStringGetCString(s, buf.as_mut_ptr(), buf.len() as isize, UTF8) } != 0;
    ok.then(|| buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8 as char).collect())
}

fn dock_pid() -> Option<i32> {
    unsafe {
        let cls = objc2::runtime::AnyClass::get(c"NSRunningApplication")?;
        let bundle = cfstr(c"com.apple.dock");
        let arr: *mut AnyObject = msg_send![cls, runningApplicationsWithBundleIdentifier: bundle as *const AnyObject];
        CFRelease(bundle);
        let arr = arr.as_ref()?;
        let n: usize = msg_send![arr, count];
        if n == 0 {
            return None;
        }
        let app: *mut AnyObject = msg_send![arr, objectAtIndex: 0usize];
        let pid: i32 = msg_send![app.as_ref()?, processIdentifier];
        Some(pid)
    }
}

/// Rettangolo della barra del Dock (l'elemento AXList dell'app Dock), in punti globali.
fn dock_rect_ax() -> Option<GRect> {
    let pid = dock_pid()?;
    unsafe {
        let app = AXUIElementCreateApplication(pid);
        if app.is_null() {
            return None;
        }
        let children = ax_attr(app, c"AXChildren");
        let mut found = None;
        if !children.is_null() {
            for i in 0..CFArrayGetCount(children) {
                let el = CFArrayGetValueAtIndex(children, i);
                let role = ax_attr(el, c"AXRole");
                let is_list = !role.is_null() && cf_to_string(role).as_deref() == Some("AXList");
                if !role.is_null() {
                    CFRelease(role);
                }
                if !is_list {
                    continue;
                }
                let pos = ax_attr(el, c"AXPosition");
                let size = ax_attr(el, c"AXSize");
                let (mut p, mut s) = (CgPoint::default(), CgPoint::default());
                let ok = !pos.is_null()
                    && !size.is_null()
                    && AXValueGetValue(pos, AX_VALUE_CGPOINT, &mut p as *mut _ as *mut c_void) != 0
                    && AXValueGetValue(size, AX_VALUE_CGSIZE, &mut s as *mut _ as *mut c_void) != 0;
                for v in [pos, size] {
                    if !v.is_null() {
                        CFRelease(v);
                    }
                }
                if ok && s.x > 0.0 && s.y > 0.0 {
                    found = Some(GRect { x: p.x, y: p.y, w: s.x, h: s.y });
                }
                break;
            }
            CFRelease(children);
        }
        CFRelease(app);
        found
    }
}

/// Schermi come (frame, visibleFrame) convertiti in punti globali con origine in alto a sinistra.
fn screens() -> Vec<(GRect, GRect)> {
    unsafe {
        let Some(cls) = objc2::runtime::AnyClass::get(c"NSScreen") else { return Vec::new() };
        let arr: *mut AnyObject = msg_send![cls, screens];
        let Some(arr) = arr.as_ref() else { return Vec::new() };
        let n: usize = msg_send![arr, count];
        let mut raw = Vec::new();
        for i in 0..n {
            let s: *mut AnyObject = msg_send![arr, objectAtIndex: i];
            let Some(s) = s.as_ref() else { continue };
            let f: NSRect = msg_send![s, frame];
            let v: NSRect = msg_send![s, visibleFrame];
            raw.push((f, v));
        }
        // Le coordinate AppKit partono dal basso dello schermo principale (il primo).
        let main_h = raw.first().map(|(f, _)| f.size.height).unwrap_or(0.0);
        let flip = |r: NSRect| GRect { x: r.origin.x, y: main_h - (r.origin.y + r.size.height), w: r.size.width, h: r.size.height };
        raw.into_iter().map(|(f, v)| (flip(f), flip(v))).collect()
    }
}

pub fn dock_info() -> Option<DockInfo> {
    let ax = if ax_trusted(false) { dock_rect_ax() } else { None };
    crate::indicators::choose_dock(&screens(), ax, dock_autohide())
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFPreferencesCopyAppValue(key: *const c_void, app: *const c_void) -> *const c_void;
    fn CFBooleanGetValue(b: *const c_void) -> u8;
    fn CFGetTypeID(v: *const c_void) -> usize;
    fn CFBooleanGetTypeID() -> usize;
}

/// Il Dock si nasconde automaticamente? (preferenza "autohide" di com.apple.dock)
fn dock_autohide() -> bool {
    let (key, app) = (cfstr(c"autohide"), cfstr(c"com.apple.dock"));
    let v = unsafe { CFPreferencesCopyAppValue(key, app) };
    let on = !v.is_null() && unsafe { CFGetTypeID(v) == CFBooleanGetTypeID() && CFBooleanGetValue(v) != 0 };
    unsafe {
        if !v.is_null() {
            CFRelease(v);
        }
        CFRelease(key);
        CFRelease(app);
    }
    on
}

