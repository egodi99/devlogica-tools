//! Esportazione di immagini statiche, senza finestre:
//!   devlogica-wallpaper --snapshot <sfondo> <larghezza> <altezza> <secondi> <file.png> [--no-logo] [--tile x y w h] [--scale s]
//! Con --tile si disegna solo una porzione della tela, come fa uno schermo in modalità estesa.

use crate::render::{self, Gpu, Params};
use crate::wallpapers;

/// Anteprima delle finestre con dati di esempio:
///   --ui <call|caffe|conferma|impostazioni> <file.png>
///   --ui statistiche <file.png> [giorno|mese|anno]   (senza argomento: settimana)
pub fn run_ui(args: &[String]) -> i32 {
    use crate::config::{now_ms, Config};
    use crate::detect::MicUser;
    use crate::office::Office;
    use crate::ui;
    let (Some(kind), Some(out)) = (args.first(), args.get(1)) else {
        eprintln!("uso: --ui <call|caffe|conferma|menu[:caffe|:sfondi]|impostazioni[:call|:avvisi|:sfondi|:mie|:diagnostica|:info]|statistiche> <file.png> [giorno|mese|anno]");
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
    office.handle(crate::net::Packet::CoffeeRsvp { user: "Giulia".into(), answer: crate::office::IN_CALL.into() }, ip, "Emanuele");
    office.peers.insert("Marco".into(), ip);
    office.peers.insert("Anna".into(), ip);
    let mic = vec![
        MicUser { id: "slack.exe".into(), label: "Slack".into(), known_call_app: true },
        MicUser { id: "obs64.exe".into(), label: "obs64".into(), known_call_app: false },
    ];
    let peers: Vec<String> = office.peers.keys().cloned().collect();
    let upd = crate::update::UpdateState::Available(crate::update::Release {
        tag: "v1.1.0".into(),
        page: String::new(),
        asset: Some(crate::update::Asset { url: String::new(), size: 0, sha256: None }),
    });
    // "Le mie call": tratti di esempio nell'ultimo anno, più una call in corso
    use crate::callstats::{Period, PeriodKind};
    let today = chrono::Local::now().date_naive();
    let mut delete = ui::DeleteForm::new(today);
    let first_call_day = example_calls(t).iter().map(|s| crate::callstats::local_date(s.start_ms, &chrono::Local)).min();
    // pagina delle impostazioni: "impostazioni:sfondi" ecc.
    let (kind, page_name) = kind.split_once(':').unwrap_or((kind.as_str(), ""));
    let mut page = match page_name {
        "call" => ui::Page::Calls,
        "avvisi" => ui::Page::Alerts,
        "sfondi" => ui::Page::Wallpaper,
        "mie" => ui::Page::MyCalls,
        "diagnostica" => ui::Page::Diagnostics,
        "info" => ui::Page::About,
        _ => ui::Page::Home,
    };
    let mut tray_page = match page_name {
        "caffe" => ui::TrayPage::Coffee,
        "sfondi" => ui::TrayPage::Wallpaper,
        _ => ui::TrayPage::Main,
    };
    let catalog = crate::wallpapers::catalog();
    let mut thumbs = Vec::new();
    if page == ui::Page::Wallpaper {
        let mut g2 = Gpu::new(render::new_instance(), None).ok();
        for wp in &catalog {
            let params = Params { res: [384.0, 216.0], canvas: [384.0, 216.0], time: 24.0, since: 60.0, logo_on: 1.0, scale: 1.0, ..Default::default() };
            if let Some(px) = g2.as_mut().and_then(|g| crate::dash::thumbnail(g, wp, params)) {
                thumbs.push((wp.id.clone(), std::sync::Arc::new(egui::ColorImage::from_rgba_unmultiplied([384, 216], &px))));
            }
        }
    }
    let monitors = vec![crate::tray::MonitorInfo { key: "a".into(), label: "DELL U2723QE (3840×2160)".into() }];
    let day_rep = crate::callstats::report(&example_calls(t), Period::containing(PeriodKind::Day, today), today, &chrono::Local);
    let week_rep = crate::callstats::report(&example_calls(t), Period::containing(PeriodKind::Week, today), today, &chrono::Local);
    cfg.wallpaper = "cruscotto".into();
    let mut view = ui::View {
        cfg: &mut cfg,
        office: &office,
        me: "Emanuele".into(),
        mic: &mic,
        ask: Some("Microsoft Teams"),
        update: &upd,
        peers,
        ax_trusted: false,
        delete: &mut delete,
        first_call_day,
        page: &mut page,
        tray_page: &mut tray_page,
        week: Some(&week_rep),
        today_ms: day_rep.total_ms,
        autostart: true,
        catalog: &catalog,
        monitors: &monitors,
        thumbs: &thumbs,
        actions: Vec::new(),
    };
    let kind_stats = match args.get(2).map(String::as_str) {
        Some("giorno") => PeriodKind::Day,
        Some("mese") => PeriodKind::Month,
        Some("anno") => PeriodKind::Year,
        _ => PeriodKind::Week,
    };
    let period = Period::containing(kind_stats, today);
    let rep = crate::callstats::report(&example_calls(t), period, today, &chrono::Local);
    let ppp = 2.0;
    let popup_h = crate::traymenu::height(&view);
    let (w, h, bg): (f64, f64, _) = match kind {
        "call" => (540.0, ui::call_panel_height(3), ui::PANEL_BG),
        "caffe" => (420.0, ui::coffee_panel_height(3, false), ui::PANEL_BG),
        "conferma" => (400.0, 132.0, ui::PANEL_BG),
        "statistiche" => (560.0, 640.0, ui::INK),
        "menu" => (crate::traymenu::WIDTH, popup_h, ui::PANEL_BG),
        _ => (980.0, 700.0, ui::INK),
    };
    let (pw, ph) = ((w * ppp as f64) as u32, (h * ppp as f64) as u32);
    let px = ui::render_preview(&gpu, pw, ph, ppp, bg, |u| match kind {
        "call" => ui::call_panel(u, &mut view),
        "caffe" => ui::coffee_panel(u, &mut view),
        "conferma" => ui::ask_panel(u, &mut view),
        "menu" => crate::traymenu::popup(u, &mut view),
        "statistiche" => {
            let _ = crate::statsui::stats_panel(u, period, today, &rep, true, true);
        }
        _ => crate::settingsui::settings(u, &mut view),
    });
    match image::save_buffer(out, &px, pw, ph, image::ExtendedColorType::Rgba8) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{e}");
            1
        }
    }
}

/// Call di esempio per l'anteprima: giorni lavorativi dell'ultimo anno con Teams, Slack e
/// Meet (durate pseudo-casuali ma fisse), più una call Teams in corso da 25 minuti.
fn example_calls(now: f64) -> Vec<crate::callstats::Segment> {
    use chrono::{Datelike, TimeZone};
    let today = chrono::Local::now().date_naive();
    let mut v = Vec::new();
    let mut seed: u32 = 7;
    let mut rnd = move || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (seed >> 16) % 1000
    };
    for back in (0..400).rev() {
        let d = today - chrono::Duration::days(back);
        if d.weekday().num_days_from_monday() >= 5 {
            continue;
        }
        for (app, h, m, base, spread) in [("Microsoft Teams", 9, 30, 30, 90), ("Slack", 14, 0, 10, 50), ("Google Meet (Chrome)", 16, 15, 0, 45)] {
            let min = base + rnd() * spread / 1000;
            if min < 5 {
                continue;
            }
            let Some(start) = chrono::Local.from_local_datetime(&d.and_hms_opt(h, m, 0).expect("ora valida")).earliest() else { continue };
            let start_ms = start.timestamp_millis() as f64;
            let end_ms = start_ms + min as f64 * 60_000.0;
            if end_ms < now - 30.0 * 60_000.0 {
                v.push(crate::callstats::Segment { start_ms, end_ms, app: app.into(), auto: true });
            }
        }
    }
    v.push(crate::callstats::Segment { start_ms: now - 25.0 * 60_000.0, end_ms: now, app: "Microsoft Teams".into(), auto: true });
    v
}

pub fn run(args: &[String]) -> i32 {
    if args.len() < 5 {
        eprintln!("uso: --snapshot <sfondo> <larghezza> <altezza> <secondi> <file.png> [--no-logo] [--scale s] [--indicatori text|pills|big_clock] [--testo k] [--logo k]");
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
        dash: [0.0; 4],
    };
    // Indicatori con dati di esempio, come sullo schermo con il Dock (Dock al centro, largo il 47%).
    let style = args.iter().position(|a| a == "--indicatori").and_then(|i| args.get(i + 1)).map(|s| match s.as_str() {
        "pills" => crate::config::IndicatorStyle::Pills,
        "big_clock" => crate::config::IndicatorStyle::BigClock,
        _ => crate::config::IndicatorStyle::Text,
    });
    let num = |flag: &str| args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).and_then(|s| s.parse::<f32>().ok());
    let ind = crate::config::Indicators {
        style: style.unwrap_or(crate::config::IndicatorStyle::Text),
        text_scale: num("--testo").unwrap_or(1.0),
        logo_scale: num("--logo").unwrap_or(1.0),
        ..Default::default()
    };
    let mut overlay = style.map(|_| crate::overlay::Overlay::new(&gpu, gpu.format));
    let ppp = 2.0 * params.scale;
    let (sw, sh) = (width as f32 / 2.0, height as f32 / 2.0);
    let frame = crate::overlay::Frame {
        ind: &ind,
        stats: crate::sysstats::Stats { cpu: 23.0, ram_used: 12_025_908_428, ram_total: 17_179_869_184 },
        hm: (14, 32),
        dock: crate::indicators::DockPlace::Bottom { x0: sw * 0.265, x1: sw * 0.735, top: sh - 64.0 },
        opacity: 1.0,
    };
    let size_px = [(width as f32 * params.scale).round() as u32, (height as f32 * params.scale).round() as u32];
    let mut draw_ind = |g: &Gpu, enc: &mut wgpu::CommandEncoder, view: &wgpu::TextureView| {
        if let Some(o) = overlay.as_mut() {
            o.draw(g, enc, view, size_px, ppp, &frame);
        }
    };
    let extra: Option<render::Extra> = if style.is_some() { Some(&mut draw_ind) } else { None };
    let px = if w.id == crate::dash::ID {
        crate::dash::render_static(&mut gpu, w, params, crate::dash::demo_stats())
    } else {
        gpu.render_offscreen(w, params, extra)
    };
    let Some(px) = px else {
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

/// Tempo medio per fotogramma di uno sfondo (solo shader), fuori schermo:
///   --bench <sfondo> [larghezza altezza]
pub fn bench(args: &[String]) -> i32 {
    let cat = wallpapers::catalog();
    let Some(w) = args.first().and_then(|id| cat.iter().find(|w| &w.id == id)) else {
        eprintln!("uso: --bench <sfondo> [larghezza altezza]");
        return 2;
    };
    let (width, height): (f32, f32) = (args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1920.0), args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1080.0));
    let Ok(mut gpu) = Gpu::new(render::new_instance(), None) else { return 1 };
    let mut params = Params { res: [width, height], canvas: [width, height], since: 60.0, logo_on: 1.0, scale: 1.0, ..Default::default() };
    let _ = gpu.render_offscreen(w, params, None);
    let n = 30;
    let t0 = std::time::Instant::now();
    for i in 0..n {
        params.time = i as f32 * 0.033;
        let _ = gpu.render_offscreen(w, params, None);
    }
    println!("{}: {:.1} ms/fotogramma a {}x{} (compresa la lettura dei pixel)", w.id, t0.elapsed().as_secs_f64() * 1000.0 / n as f64, width, height);
    0
}
