//! Parti specifiche del sistema operativo: posizionare le finestre "dietro le icone",
//! rilevare le app a schermo intero, avvio automatico.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;
#[cfg(windows)]
pub mod probe_windows;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use self::macos::*;

// Su altri sistemi (es. Linux) l'app compila ma le finestre restano normali: utile solo per sviluppo.
#[cfg(not(any(windows, target_os = "macos")))]
mod other {
    use winit::dpi::{PhysicalPosition, PhysicalSize};
    use winit::window::Window;
    pub fn attach(_w: &Window, _pos: PhysicalPosition<i32>, _size: PhysicalSize<u32>) -> bool { true }
    pub fn desktop_lost() -> bool { false }
    pub fn fullscreen_busy() -> bool { false }
    pub fn restore_desktop() {}
    pub fn autostart_enabled() -> bool { false }
    pub fn set_autostart(_on: bool) -> Result<(), String> { Err("non supportato".into()) }
    /// Solo per prove su Linux: DLT_FAKE_MIC="id app" simula un microfono in uso.
    pub fn mic_users() -> Vec<crate::detect::RawMicUser> {
        std::env::var("DLT_FAKE_MIC").ok().filter(|s| !s.is_empty()).map(|id| vec![crate::detect::RawMicUser { id, titles: Vec::new() }]).unwrap_or_default()
    }
    pub fn make_overlay(_w: &Window) {}
    pub fn open_url(url: &str) { let _ = std::process::Command::new("xdg-open").arg(url).spawn(); }
    pub fn pointer() -> Option<(f64, f64, bool)> { None }
    pub fn desktop_at(_x: f64, _y: f64) -> bool { false }
}

// Aspetto delle finestre e area di lavoro: solo Windows.
#[cfg(not(windows))]
#[allow(dead_code)]
mod no_dwm {
    use winit::window::Window;
    pub fn style_window(_w: &Window) {}
    pub fn style_popup(_w: &Window, _border: egui::Color32) {}
    pub fn work_area(_x: i32, _y: i32) -> Option<(i32, i32, i32, i32)> { None }
}
#[cfg(not(windows))]
pub use no_dwm::*;
#[cfg(not(any(windows, target_os = "macos")))]
pub use other::*;

// Statistiche per gli indicatori: esistono solo su macOS.
#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
mod no_stats {
    pub fn cpu_ticks() -> Option<[u64; 4]> { None }
    pub fn vm_pages() -> Option<(u64, u64, u64, u64, u64)> { None }
    pub fn mem_total() -> Option<u64> { None }
    pub fn local_hm() -> (u32, u32) { (0, 0) }
    pub fn ax_trusted(_prompt: bool) -> bool { false }
    pub fn dock_info() -> Option<crate::indicators::DockInfo> { None }
    pub fn open_accessibility_settings() {}
}
#[cfg(not(target_os = "macos"))]
#[allow(unused_imports)]
pub use no_stats::*;

/// Apre una cartella nel file manager del sistema.
pub fn open_folder(path: &std::path::Path) {
    let _ = std::fs::create_dir_all(path);
    #[cfg(windows)]
    let cmd = "explorer";
    #[cfg(target_os = "macos")]
    let cmd = "open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let cmd = "xdg-open";
    let _ = std::process::Command::new(cmd).arg(path).spawn();
}

/// Programmi e pannelli di sistema aperti dai tasti del cruscotto.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SysTool {
    TaskManager,
    Docker,
    Wsl,
    Network,
    Power,
    Clock,
    Lock,
}

/// Apre un programma o un pannello di sistema (quelli che non esistono sul sistema non fanno nulla).
pub fn open_tool(t: SysTool) {
    use std::process::Command;
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const NO_WINDOW: u32 = 0x0800_0000;
        let start = |target: &str| {
            let _ = Command::new("cmd").args(["/C", "start", "", target]).creation_flags(NO_WINDOW).spawn();
        };
        match t {
            SysTool::TaskManager => {
                let _ = Command::new("taskmgr").spawn();
            }
            SysTool::Docker => {
                let pf = std::env::var("ProgramFiles").unwrap_or_else(|_| "C:\\Program Files".into());
                let exe = std::path::Path::new(&pf).join("Docker").join("Docker").join("Docker Desktop.exe");
                if exe.exists() {
                    let _ = Command::new(exe).spawn();
                }
            }
            // nuova finestra del terminale con la distribuzione WSL predefinita
            SysTool::Wsl => {
                let _ = Command::new("cmd").args(["/C", "start", "", "wsl.exe", "--cd", "~"]).creation_flags(NO_WINDOW).spawn();
            }
            SysTool::Network => start("ms-settings:network-status"),
            SysTool::Power => start("ms-settings:powersleep"),
            SysTool::Clock => start("ms-clock:"),
            SysTool::Lock => {
                let _ = Command::new("rundll32.exe").args(["user32.dll,LockWorkStation"]).spawn();
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let open = |args: &[&str]| {
            let _ = Command::new("open").args(args).spawn();
        };
        match t {
            SysTool::TaskManager => open(&["-a", "Activity Monitor"]),
            SysTool::Docker => open(&["-a", "Docker"]),
            SysTool::Wsl => open(&["-a", "Terminal"]),
            SysTool::Network => open(&["x-apple.systempreferences:com.apple.Network-Settings.extension"]),
            SysTool::Power => open(&["x-apple.systempreferences:com.apple.Battery-Settings.extension"]),
            SysTool::Clock => open(&["-a", "Clock"]),
            // spegne lo schermo: con "richiedi password subito" (predefinito) blocca il Mac
            SysTool::Lock => {
                let _ = Command::new("pmset").arg("displaysleepnow").spawn();
            }
        }
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (t, Command::new("true"));
    }
}
