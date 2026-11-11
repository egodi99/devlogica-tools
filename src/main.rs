//! DevLogica Tools — sfondi animati, avvisi di call e pausa caffè per l'ufficio.
//! Windows e macOS, leggerissimo: shader GPU per gli sfondi, rete locale senza server.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod callstats;
mod callstore;
mod config;
mod detect;
mod indicators;
mod install;
mod net;
mod notify;
mod office;
mod overlay;
mod platform;
mod render;
mod settingsui;
mod dash;
mod theme;
mod traymenu;
mod snapshot;
mod statsui;
mod sound;
mod sysstats;
mod tray;
mod ui;
mod update;
mod wallpaper;
mod wallpapers;

use winit::event_loop::EventLoop;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(|a| a == "--snapshot").unwrap_or(false) {
        std::process::exit(snapshot::run(&args[2..]));
    }
    if args.get(1).map(|a| a == "--notifica-prova").unwrap_or(false) {
        std::process::exit(notify::run_test());
    }
    if args.get(1).map(|a| a == "--bench").unwrap_or(false) {
        std::process::exit(snapshot::bench(&args[2..]));
    }
    if args.get(1).map(|a| a == "--statistiche").unwrap_or(false) {
        // letture del cruscotto per qualche secondo, per verificare i contatori su questo PC
        let s = dash::DashSampler::start();
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            println!("{:?}", s.latest());
        }
        return;
    }
    if args.get(1).map(|a| a == "--ui").unwrap_or(false) {
        std::process::exit(snapshot::run_ui(&args[2..]));
    }

    // Una sola copia in esecuzione: la seconda si chiude subito. Dopo un aggiornamento invece
    // si aspetta (fino a 30 s) che la versione precedente si chiuda e liberi la porta.
    let after_update = args.iter().any(|a| a == "--dopo-aggiornamento");
    let bind = || std::net::TcpListener::bind(("127.0.0.1", 47_824));
    let mut lock = bind();
    if after_update {
        let until = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while lock.is_err() && std::time::Instant::now() < until {
            std::thread::sleep(std::time::Duration::from_millis(250));
            lock = bind();
        }
    }
    let Ok(_guard) = lock else {
        config::log("DevLogica Tools è già in esecuzione");
        return;
    };
    if after_update {
        config::log(format!("Aggiornato alla versione {}", env!("CARGO_PKG_VERSION")));
    }
    install::cleanup();

    let mut builder = EventLoop::<app::UserEvent>::with_user_event();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
        // Nessuna icona nel Dock: l'app vive solo nella barra dei menu.
        builder.with_activation_policy(ActivationPolicy::Accessory);
    }
    let event_loop = builder.build().expect("event loop");
    let proxy = event_loop.create_proxy();
    let menu_proxy = proxy.clone();
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |e: tray_icon::menu::MenuEvent| {
        let _ = menu_proxy.send_event(app::UserEvent::Menu(e.id));
    }));

    // Windows: il clic sull'icona apre il menu a comparsa (traymenu) invece del menu nativo.
    #[cfg(windows)]
    {
        let tray_proxy = proxy.clone();
        tray_icon::TrayIconEvent::set_event_handler(Some(move |e: tray_icon::TrayIconEvent| {
            if let tray_icon::TrayIconEvent::Click { position, button_state: tray_icon::MouseButtonState::Up, .. } = e {
                let _ = tray_proxy.send_event(app::UserEvent::TrayClick { x: position.x, y: position.y });
            }
        }));
    }

    let mut app = app::App::new(proxy);
    if let Err(e) = event_loop.run_app(&mut app) {
        config::log(format!("Errore nel ciclo eventi: {e}"));
    }
    platform::restore_desktop();
}
