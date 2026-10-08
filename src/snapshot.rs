//! Esportazione di immagini statiche, senza finestre:
//!   devlogica-wallpaper --snapshot <sfondo> <larghezza> <altezza> <secondi> <file.png> [--no-logo] [--tile x y w h] [--scale s]
//! Con --tile si disegna solo una porzione della tela, come fa uno schermo in modalità estesa.

use crate::render::{self, Gpu, Params};
use crate::wallpapers;

/// Anteprima delle finestre con dati di esempio:  --ui <call|caffe|conferma|impostazioni> <file.png>
pub fn run_ui(args: &[String]) -> i32 {
    use crate::config::{now_ms, Config};
    use crate::detect::MicUser;
    use crate::office::Office;
    use crate::ui;
    let (Some(kind), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("uso: --ui <call|caffe|conferma|impostazioni> <file.png>");
        return 2;
    };
    let Ok(gpu) = Gpu::new(render::new_instance(), None) else { return 1 };
    let mut cfg = Config { name: "Emanuele".into(), ..Default::default() };
    let mut office = Office::default();
    let ip: std::net::IpAddr = [192, 168, 1, 20].into();
    let t = now_ms();
    office.handle(crate::net::Packet::CallStart { call_id: "m".into(), user: "Marco".into(), start_time: t - 754_000.0, app: Some("Microsoft Teams".into()), auto: true }, ip, "Emanuele");
    office.handle(crate::net::Packet::CallStart { call_id: "a".into(), user: "Anna".into(), start_time: t - 95_000.0, app: Some("Google Meet (Chrome)".into()), auto: true }, ip, "Emanuele");
    office.start_my_call("Emanuele", Some("Slack".into()), true);
    office.handle(crate::net::Packet::CoffeeStart { user: "Marco".into(), in_minutes: Some(5) }, ip, "Emanuele");
    office.handle(crate::net::Packet::CoffeeRsvp { user: "Anna".into(), answer: "yes".into() }, ip, "Emanuele");
    office.handle(crate::net::Packet::CoffeeRsvp { user: "Luca".into(), answer: "no".into() }, ip, "Emanuele");
    office.peers.insert("Marco".into(), ip);
    office.peers.insert("Anna".into(), ip);
    let mic = vec![
        MicUser { id: "slack.exe".into(), label: "Slack".into(), known_call_app: true },
        MicUser { id: "obs64.exe".into(), label: "obs64".into(), known_call_app: false },
    ];
    let peers: Vec<String> = office.peers.keys().cloned().collect();
    let mut view = ui::View { cfg: &mut cfg, office: &office, me: "Emanuele".into(), mic: &mic, ask: Some("Microsoft Teams"), update: Some("v1.1.0"), peers, actions: Vec::new() };
    let ppp = 2.0;
    let (w, h, bg): (f64, f64, _) = match kind.as_str() {
        "call" => (540.0, ui::call_panel_height(3), ui::RED_BG),
        "caffe" => (420.0, ui::coffee_panel_height(2), ui::COFFEE_BG),
        "conferma" => (380.0, 140.0, ui::INK),
        _ => (500.0, 680.0, ui::INK),
    };
    let (pw, ph) = ((w * ppp as f64) as u32, (h * ppp as f64) as u32);
    let px = ui::render_preview(&gpu, pw, ph, ppp, bg, |u| match kind.as_str() {
        "call" => ui::call_panel(u, &mut view),
        "caffe" => ui::coffee_panel(u, &mut view),
        "conferma" => ui::ask_panel(u, &mut view),
        _ => ui::settings(u, &mut view),
    });
    match image::save_buffer(out, &px, pw, ph, image::ExtendedColorType::Rgba8) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

pub fn run(args: &[String]) -> i32 {
    if args.len() < 5 {
        eprintln!("uso: --snapshot <sfondo> <larghezza> <altezza> <secondi> <file.png> [--no-logo]");
        eprintln!("sfondi: {}", wallpapers::catalog().iter().map(|w| w.id.clone()).collect::<Vec<_>>().join(", "));
        return 2;
    }
    let cat = wallpapers::catalog();
    let Some(w) = cat.iter().find(|w| w.id == args[0]) else {
        eprintln!("sfondo sconosciuto: {}", args[0]);
        return 2;
    };
    let (width, height): (u32, u32) = (args[1].parse().unwrap_or(1920), args[2].parse().unwrap_or(1080));
    let t: f32 = args[3].parse().unwrap_or(20.0);
    let logo = !args.iter().any(|a| a == "--no-logo");

    let mut gpu = match Gpu::new(render::new_instance(), None) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}");
            return 1;
        }
    };
    let tile: Option<[u32; 4]> = args.iter().position(|a| a == "--tile").and_then(|i| {
        let v: Vec<u32> = args.get(i + 1..i + 5)?.iter().filter_map(|s| s.parse().ok()).collect();
        (v.len() == 4).then(|| [v[0], v[1], v[2], v[3]])
    });
    let [ox, oy, tw, th] = tile.unwrap_or([0, 0, width, height]);
    let (width, height, canvas) = (tw, th, [width as f32, height as f32]);
    let params = Params {
        res: [width as f32, height as f32],
        offset: [ox as f32, oy as f32],
        canvas,
        time: t,
        since: 60.0,
        logo_on: if logo { 1.0 } else { 0.0 },
        scale: args.iter().position(|a| a == "--scale").and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(1.0),
        _pad: [0.0; 2],
    };
    let Some(px) = gpu.render_offscreen(w, params) else {
        eprintln!("rendering non riuscito (errore nello shader?)");
        return 1;
    };
    let (width, height) = ((width as f32 * params.scale).round() as u32, (height as f32 * params.scale).round() as u32);
    match image::save_buffer(&args[4], &px, width, height, image::ExtendedColorType::Rgba8) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}
