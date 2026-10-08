//! Icona nella barra di sistema (Windows) / barra dei menu (macOS) con il menu principale.

use crate::config::{Config, Mode, Quality};
use crate::office::Office;
use crate::wallpapers::Wallpaper;
use std::collections::HashMap;
use tray_icon::menu::{CheckMenuItem, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

#[derive(Clone, Debug)]
pub enum Action {
    StartCall,
    EndCall,
    ShowCalls,
    Coffee(Option<u32>),
    EndCoffee,
    ShowCoffee,
    OpenSettings,
    OpenUpdate,
    // sfondi
    ToggleWallpaper,
    SetMode(Mode),
    SetWallpaper(String),
    SetMonitorWallpaper { monitor: String, wallpaper: String },
    SetFps(u32),
    SetQuality(Quality),
    ToggleLogo,
    TogglePause,
    #[cfg_attr(not(windows), allow(dead_code))]
    ToggleFullscreenPause,
    ToggleAutostart,
    OpenFolder,
    Reload,
    Quit,
}

pub struct MonitorInfo {
    pub key: String,
    pub label: String,
}

pub struct Built {
    pub menu: Menu,
    pub actions: HashMap<MenuId, Action>,
}

pub struct MenuState<'a> {
    pub cfg: &'a Config,
    pub office: &'a Office,
    pub catalog: &'a [Wallpaper],
    pub monitors: &'a [MonitorInfo],
    pub autostart: bool,
    pub update: Option<&'a str>,
    pub calls_hidden: bool,
    pub coffee_hidden: bool,
}

pub fn build(s: &MenuState) -> Built {
    let cfg = s.cfg;
    let mut actions = HashMap::new();
    let menu = Menu::new();
    let mut item = |text: &str, enabled: bool, action: Option<Action>| {
        let it = MenuItem::new(text, enabled, None);
        if let Some(a) = action {
            actions.insert(it.id().clone(), a);
        }
        it
    };
    let mut checks: Vec<(CheckMenuItem, Action)> = Vec::new();
    let mut check = |text: &str, on: bool, action: Action| {
        let it = CheckMenuItem::new(text, true, on, None);
        checks.push((it.clone(), action));
        it
    };

    // ---- stato ----
    let others: Vec<String> = s.office.others().iter().map(|c| c.user.clone()).collect();
    let status = match (others.is_empty(), s.office.my_call.is_some()) {
        (true, false) => "Nessuna call in corso".to_string(),
        (true, true) => "Sei in call".to_string(),
        (false, mine) => format!("In call: {}{}", others.join(", "), if mine { " e tu" } else { "" }),
    };
    let _ = menu.append(&item("DevLogica Tools", false, None));
    let _ = menu.append(&item(&status, false, None));
    let _ = menu.append(&PredefinedMenuItem::separator());

    // ---- call ----
    match &s.office.my_call {
        None => {
            let _ = menu.append(&item("Sto entrando in call", true, Some(Action::StartCall)));
        }
        Some(m) => {
            let label = if m.auto { "Termina la mia call (rilevata in automatico)" } else { "Termina la mia call" };
            let _ = menu.append(&item(label, true, Some(Action::EndCall)));
        }
    }
    if !others.is_empty() && s.calls_hidden {
        let _ = menu.append(&item("Mostra chi è in call", true, Some(Action::ShowCalls)));
    }

    // ---- caffè ----
    match &s.office.coffee {
        Some(c) if c.mine => {
            let _ = menu.append(&item("Fine pausa caffè", true, Some(Action::EndCoffee)));
        }
        Some(c) => {
            if s.coffee_hidden {
                let _ = menu.append(&item(&format!("Pausa caffè di {}…", c.by), true, Some(Action::ShowCoffee)));
            }
        }
        None => {
            let sub = Submenu::new("Pausa caffè", true);
            for (label, m) in [("Adesso", None), ("Tra 5 minuti", Some(5)), ("Tra 10 minuti", Some(10)), ("Tra 15 minuti", Some(15))] {
                let _ = sub.append(&item(label, true, Some(Action::Coffee(m))));
            }
            let _ = menu.append(&sub);
        }
    }
    let _ = menu.append(&PredefinedMenuItem::separator());

    // ---- sfondi ----
    let wall = Submenu::new("Sfondi animati", true);
    let _ = wall.append(&check("Attivi", cfg.wallpaper_enabled, Action::ToggleWallpaper));
    let _ = wall.append(&PredefinedMenuItem::separator());
    let modes = Submenu::new("Schermi", cfg.wallpaper_enabled);
    for (m, label) in [
        (Mode::Same, "Stesso sfondo su ogni schermo"),
        (Mode::PerMonitor, "Uno sfondo diverso per schermo"),
        (Mode::Span, "Un unico sfondo esteso su tutti"),
    ] {
        let _ = modes.append(&check(label, cfg.mode == m, Action::SetMode(m)));
    }
    let _ = wall.append(&modes);
    if cfg.mode == Mode::PerMonitor {
        for (i, mon) in s.monitors.iter().enumerate() {
            let current = cfg.per_monitor.get(&mon.key).unwrap_or(&cfg.wallpaper);
            let sub = Submenu::new(format!("Schermo {} — {}", i + 1, mon.label), cfg.wallpaper_enabled);
            for w in s.catalog {
                let _ = sub.append(&check(
                    &w.name,
                    *current == w.id,
                    Action::SetMonitorWallpaper { monitor: mon.key.clone(), wallpaper: w.id.clone() },
                ));
            }
            let _ = wall.append(&sub);
        }
    } else {
        let sub = Submenu::new("Sfondo", cfg.wallpaper_enabled);
        for w in s.catalog {
            let _ = sub.append(&check(&w.name, cfg.wallpaper == w.id, Action::SetWallpaper(w.id.clone())));
        }
        let _ = wall.append(&sub);
    }
    let fps = Submenu::new("Fluidità", cfg.wallpaper_enabled);
    for (n, label) in [(15, "15 fps — consumo minimo"), (24, "24 fps"), (30, "30 fps — consigliato"), (60, "60 fps")] {
        let _ = fps.append(&check(label, cfg.fps() == n, Action::SetFps(n)));
    }
    let _ = wall.append(&fps);
    let quality = Submenu::new("Risoluzione", cfg.wallpaper_enabled);
    for (q, label) in [(Quality::Auto, "Automatica — consigliata"), (Quality::Full, "Piena"), (Quality::Low, "Ridotta — memoria minima")] {
        let _ = quality.append(&check(label, cfg.quality == q, Action::SetQuality(q)));
    }
    let _ = wall.append(&quality);
    let _ = wall.append(&check("Mostra il logo", cfg.show_logo, Action::ToggleLogo));
    let _ = wall.append(&check("In pausa", cfg.paused, Action::TogglePause));
    #[cfg(windows)]
    let _ = wall.append(&check("Pausa con app a schermo intero", cfg.pause_on_fullscreen, Action::ToggleFullscreenPause));
    let _ = wall.append(&PredefinedMenuItem::separator());
    let _ = wall.append(&item("Apri cartella sfondi personalizzati", true, Some(Action::OpenFolder)));
    let _ = wall.append(&item("Ricarica sfondi", true, Some(Action::Reload)));
    let _ = menu.append(&wall);

    // ---- generale ----
    let _ = menu.append(&item("Impostazioni…", true, Some(Action::OpenSettings)));
    let _ = menu.append(&check("Avvia all'accesso", s.autostart, Action::ToggleAutostart));
    let _ = menu.append(&PredefinedMenuItem::separator());
    if let Some(tag) = s.update {
        let _ = menu.append(&item(&format!("Scarica la nuova versione {tag}"), true, Some(Action::OpenUpdate)));
    }
    let _ = menu.append(&item(&format!("Versione {}", env!("CARGO_PKG_VERSION")), false, None));
    let _ = menu.append(&item("Esci", true, Some(Action::Quit)));

    for (it, a) in checks {
        actions.insert(it.id().clone(), a);
    }
    Built { menu, actions }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Badge {
    None,
    Call,
    Coffee,
}

/// Icona con un pallino colorato: rosso se qualcuno è in call, ambra per la pausa caffè.
pub fn icon(badge: Badge) -> tray_icon::Icon {
    let mut img = image::load_from_memory(include_bytes!("../assets/icon.png")).expect("icona").to_rgba8();
    let (w, h) = img.dimensions();
    let color = match badge {
        Badge::None => None,
        Badge::Call => Some([255u8, 59, 48]),
        Badge::Coffee => Some([240, 170, 70]),
    };
    if let Some(c) = color {
        let r = w as f32 * 0.22;
        let (cx, cy) = (w as f32 - r - 1.0, h as f32 - r - 1.0);
        for y in 0..h {
            for x in 0..w {
                let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
                if d < r + 1.5 {
                    // bordo scuro per staccare il pallino dal logo, poi il pallino
                    let p = img.get_pixel_mut(x, y);
                    let a = (r + 1.5 - d).clamp(0.0, 1.0);
                    let col = if d < r { c } else { [20, 10, 30] };
                    for i in 0..3 {
                        p.0[i] = (p.0[i] as f32 * (1.0 - a) + col[i] as f32 * a) as u8;
                    }
                    p.0[3] = p.0[3].max((a * 255.0) as u8);
                }
            }
        }
    }
    tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("icona")
}
