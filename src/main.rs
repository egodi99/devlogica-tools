//! DevLogica Tools — sfondi animati, avvisi di call e pausa caffè per l'ufficio.
//! Windows e macOS, leggerissimo: shader GPU per gli sfondi, rete locale senza server.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod config;
mod detect;
mod net;
mod office;
mod platform;
mod render;
mod snapshot;
mod sound;
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
    if args.get(1).map(|a| a == "--ui").unwrap_or(false) {
        std::process::exit(snapshot::run_ui(&args[2..]));
    }

    // Una sola copia in esecuzione: la seconda si chiude subito.
    let Ok(_guard) = std::net::TcpListener::bind(("127.0.0.1", 47_824)) else {
        config::log("DevLogica Tools è già in esecuzione");
        return;
    };

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

    let mut app = app::App::new(proxy);
    if let Err(e) = event_loop.run_app(&mut app) {
        config::log(format!("Errore nel ciclo eventi: {e}"));
    }
    platform::restore_desktop();
}
