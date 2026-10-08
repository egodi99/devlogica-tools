//! Parti specifiche del sistema operativo: posizionare le finestre "dietro le icone",
//! rilevare le app a schermo intero, avvio automatico.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use self::windows::*;

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
}
#[cfg(not(any(windows, target_os = "macos")))]
pub use other::*;

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
