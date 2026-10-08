//! Coordinatore: sfondi, rete, riconoscimento call, finestre di avviso e menu.

use crate::config::{log, AlertStyle, Config, Detection};
use crate::detect::{Detect, Detector};
use crate::net::{Net, Packet};
use crate::office::{Notice, Office, Outgoing};
use crate::render::Gpu;
use crate::sound::{self, Chime};
use crate::tray::{self, Action, Badge, MenuState};
use crate::ui::{self, Edge, UiAction, UiWin, View};
use crate::wallpaper::Wallpapers;
use crate::wallpapers::{self as catalog, Wallpaper};
use crate::{platform, update};
use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};
use tray_icon::menu::MenuId;
use tray_icon::{TrayIcon, TrayIconBuilder};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoopProxy};
use winit::monitor::MonitorHandle;
use winit::window::WindowId;

#[derive(Debug)]
pub enum UserEvent {
    Menu(MenuId),
    Packet(Packet, IpAddr),
    Update(String, String),
}

pub struct App {
    cfg: Config,
    catalog: Vec<Wallpaper>,
    gpu: Option<Gpu>,
    wall: Wallpapers,
    proxy: EventLoopProxy<UserEvent>,
    net: Option<Net>,
    office: Office,
    detector: Detector,
    tray: Option<TrayIcon>,
    actions: HashMap<MenuId, Action>,
    menu_sig: String,
    badge: Badge,
    // finestre
    settings: Option<UiWin>,
    call_win: Option<UiWin>,
    coffee_win: Option<UiWin>,
    ask_win: Option<UiWin>,
    edges: Vec<Edge>,
    calls_hidden: bool,
    coffee_hidden: bool,
    /// App rilevata per cui stiamo chiedendo conferma.
    ask: Option<String>,
    /// L'utente ha risposto "No": non richiedere finché il microfono non si chiude.
    ask_declined: bool,
    update: Option<(String, String)>,
    // ritmi
    started: Instant,
    next_frame: Instant,
    next_check: Instant,
    next_detect: Instant,
    next_alive: Instant,
    next_edge: Instant,
    fullscreen: bool,
}

impl App {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let now = Instant::now();
        Self {
            cfg: Config::load(),
            catalog: catalog::catalog(),
            gpu: None,
            wall: Wallpapers::new(),
            proxy,
            net: None,
            office: Office::default(),
            detector: Detector::new(),
            tray: None,
            actions: HashMap::new(),
            menu_sig: String::new(),
            badge: Badge::None,
            settings: None,
            call_win: None,
            coffee_win: None,
            ask_win: None,
            edges: Vec::new(),
            calls_hidden: false,
            coffee_hidden: false,
            ask: None,
            ask_declined: false,
            update: None,
            started: now,
            next_frame: now,
            next_check: now + Duration::from_secs(2),
            next_detect: now + Duration::from_secs(2),
            next_alive: now + Duration::from_secs(20),
            next_edge: now,
            fullscreen: false,
        }
    }

    fn me(&self) -> String {
        self.cfg.display_name()
    }

    // ------------------------------------------------------------------ rete

    fn send_all(&self, p: &Packet) {
        if let Some(n) = &self.net {
            n.broadcast(p, &self.office.peers);
        }
    }

    fn send_out(&self, out: Outgoing) {
        let Some(n) = &self.net else { return };
        for (to, p) in out {
            match to {
                Some(ip) => n.send_to(&p, ip),
                None => n.broadcast(&p, &self.office.peers),
            }
        }
    }

    fn on_packet(&mut self, el: &ActiveEventLoop, p: Packet, from: IpAddr) {
        let me = self.me();
        let (notices, out) = self.office.handle(p, from, &me);
        self.send_out(out);
        for n in notices {
            match n {
                Notice::CallStarted { user } => {
                    log(format!("{user} è entrato in call"));
                    self.calls_hidden = false;
                    sound::play(Chime::Call, self.cfg.volume);
                }
                Notice::CoffeeStarted { by } => {
                    log(format!("{by} propone una pausa caffè"));
                    self.coffee_hidden = false;
                    sound::play(Chime::Coffee, self.cfg.volume);
                }
                Notice::CallsChanged | Notice::CoffeeChanged => {}
            }
        }
        self.refresh(el);
    }

    // ------------------------------------------------------------------ azioni

    fn start_call(&mut self, el: &ActiveEventLoop, app: Option<String>, auto: bool) {
        let me = self.me();
        if let Some(p) = self.office.start_my_call(&me, app.clone(), auto) {
            log(format!("Inizio call ({}){}", app.as_deref().unwrap_or("manuale"), if auto { " rilevata in automatico" } else { "" }));
            self.send_all(&p);
            self.next_alive = Instant::now() + Duration::from_secs(20);
        }
        self.refresh(el);
    }

    fn end_call(&mut self, el: &ActiveEventLoop) {
        if let Some(p) = self.office.end_my_call() {
            log("Fine call");
            self.send_all(&p);
        }
        self.refresh(el);
    }

    fn handle_action(&mut self, el: &ActiveEventLoop, action: Action) {
        let mut rebuild_wall = false;
        let me = self.me();
        match action {
            Action::StartCall => self.start_call(el, None, false),
            Action::EndCall => self.end_call(el),
            Action::ShowCalls => self.calls_hidden = false,
            Action::Coffee(min) => {
                let p = self.office.start_coffee(&me, min);
                self.coffee_hidden = false;
                self.send_all(&p);
            }
            Action::EndCoffee => {
                if let Some(p) = self.office.end_coffee(&me) {
                    self.send_all(&p);
                }
            }
            Action::ShowCoffee => self.coffee_hidden = false,
            Action::OpenSettings => self.open_settings(el),
            Action::OpenUpdate => {
                if let Some((_, url)) = &self.update {
                    platform::open_url(url);
                }
            }
            Action::ToggleWallpaper => {
                self.cfg.wallpaper_enabled = !self.cfg.wallpaper_enabled;
                rebuild_wall = true;
            }
            Action::SetMode(m) => {
                self.cfg.mode = m;
                rebuild_wall = true;
            }
            Action::SetWallpaper(id) => {
                self.cfg.wallpaper = id;
                rebuild_wall = true;
            }
            Action::SetMonitorWallpaper { monitor, wallpaper } => {
                self.cfg.per_monitor.insert(monitor, wallpaper);
                rebuild_wall = true;
            }
            Action::SetFps(n) => self.cfg.fps = n,
            Action::SetQuality(q) => {
                self.cfg.quality = q;
                rebuild_wall = true;
            }
            Action::ToggleLogo => self.cfg.show_logo = !self.cfg.show_logo,
            Action::TogglePause => self.cfg.paused = !self.cfg.paused,
            Action::ToggleFullscreenPause => self.cfg.pause_on_fullscreen = !self.cfg.pause_on_fullscreen,
            Action::ToggleAutostart => {
                if let Err(e) = platform::set_autostart(!platform::autostart_enabled()) {
                    log(format!("Avvio automatico: {e}"));
                }
                self.menu_sig.clear();
            }
            Action::OpenFolder => platform::open_folder(&crate::config::custom_wallpapers_dir()),
            Action::Reload => {
                self.catalog = catalog::catalog();
                if let Some(g) = self.gpu.as_mut() {
                    g.clear_pipelines();
                }
                rebuild_wall = true;
            }
            Action::Quit => {
                self.shutdown();
                el.exit();
                return;
            }
        }
        self.cfg.save();
        if rebuild_wall {
            self.wall.rebuild(el, &mut self.gpu, &self.cfg);
        }
        self.refresh(el);
    }

    fn handle_ui(&mut self, el: &ActiveEventLoop, actions: Vec<UiAction>, old_name: &str) {
        let me = self.me();
        for a in actions {
            match a {
                UiAction::DismissCalls => self.calls_hidden = true,
                UiAction::EndMyCall => self.end_call(el),
                UiAction::Rsvp(yes) => {
                    if let Some(p) = self.office.rsvp(&me, yes) {
                        self.send_all(&p);
                    }
                }
                UiAction::EndCoffee => {
                    if let Some(p) = self.office.end_coffee(&me) {
                        self.send_all(&p);
                    }
                }
                UiAction::DismissCoffee => self.coffee_hidden = true,
                UiAction::AskYes => {
                    let app = self.ask.take();
                    self.start_call(el, app, true);
                }
                UiAction::AskNo => {
                    self.ask = None;
                    self.ask_declined = true;
                }
                UiAction::Ignore(id) => {
                    if !self.cfg.ignored_apps.contains(&id) {
                        self.cfg.ignored_apps.push(id);
                    }
                    self.cfg.save();
                }
                UiAction::Unignore(id) => {
                    self.cfg.ignored_apps.retain(|x| *x != id);
                    self.cfg.save();
                }
                UiAction::ConfigChanged => {
                    self.cfg.save();
                    if self.me() != old_name {
                        // nome cambiato: se ero in call la ri-annuncio con il nome nuovo
                        let me = self.me();
                        for p in self.office.rename(&me) {
                            self.send_all(&p);
                        }
                        self.send_all(&Packet::Ping { user: Some(me) });
                    }
                }
                UiAction::OpenUpdate => {
                    if let Some((_, url)) = &self.update {
                        platform::open_url(url);
                    }
                }
            }
        }
        self.refresh(el);
    }

    fn shutdown(&mut self) {
        let me = self.me();
        if let Some(p) = self.office.end_my_call() {
            self.send_all(&p);
        }
        if let Some(p) = self.office.end_coffee(&me) {
            self.send_all(&p);
        }
        self.wall.clear();
        self.call_win = None;
        self.coffee_win = None;
        self.ask_win = None;
        self.settings = None;
        self.edges.clear();
    }

    // ------------------------------------------------------------------ finestre

    fn primary(el: &ActiveEventLoop) -> Option<MonitorHandle> {
        el.primary_monitor().or_else(|| el.available_monitors().next())
    }

    fn open_settings(&mut self, el: &ActiveEventLoop) {
        if let Some(s) = &self.settings {
            s.window.focus_window();
            return;
        }
        let (Some(gpu), Some(m)) = (self.gpu.as_ref(), Self::primary(el)) else { return };
        let (w, h) = ui::logical_size(&m);
        let (ww, wh) = (500.0, 680.0f64.min(h - 80.0));
        let attrs = ui::attrs_on(&m, (w - ww) / 2.0, (h - wh) / 2.0, ww, wh).with_title("DevLogica Tools – Impostazioni").with_resizable(false);
        self.settings = UiWin::new(el, gpu, attrs, false);
        if let Some(s) = &self.settings {
            s.window.focus_window();
        }
    }

    /// Mostra o nasconde le finestre di avviso in base allo stato attuale.
    fn refresh(&mut self, el: &ActiveEventLoop) {
        let others = self.office.others().len();
        let style = self.cfg.alert_style;
        let want_panel = others > 0 && !self.calls_hidden && style != AlertStyle::Tray;
        let want_edges = others > 0 && !self.calls_hidden && style == AlertStyle::Full;
        let want_coffee = self.office.coffee.is_some() && !self.coffee_hidden;
        let want_ask = self.ask.is_some();
        let primary = Self::primary(el);

        if let (Some(gpu), Some(m)) = (self.gpu.as_ref(), primary.as_ref()) {
            let (sw, sh) = ui::logical_size(m);
            // riquadro call: in alto al centro
            let call_h = ui::call_panel_height(self.office.calls.len());
            if want_panel && self.call_win.is_none() {
                let w = 540.0;
                let attrs = ui::overlay_attrs(ui::attrs_on(m, (sw - w) / 2.0, 44.0, w, call_h), "DevLogica Tools – call in corso");
                self.call_win = UiWin::new(el, gpu, attrs, true);
            }
            if let Some(cw) = &self.call_win {
                cw.set_height(call_h);
            }
            // pausa caffè: al centro
            let rows = self.office.coffee.as_ref().map(|c| c.rsvp.len().max(1)).unwrap_or(1);
            let coffee_h = ui::coffee_panel_height(rows);
            if want_coffee && self.coffee_win.is_none() {
                let w = 420.0;
                let attrs = ui::overlay_attrs(ui::attrs_on(m, (sw - w) / 2.0, (sh - coffee_h) / 2.0, w, coffee_h), "DevLogica Tools – pausa caffè");
                self.coffee_win = UiWin::new(el, gpu, attrs, true);
            }
            if let Some(cw) = &self.coffee_win {
                cw.set_height(coffee_h);
            }
            // richiesta di conferma: in basso a destra
            if want_ask && self.ask_win.is_none() {
                let (w, h) = (380.0, 140.0);
                let attrs = ui::overlay_attrs(ui::attrs_on(m, sw - w - 24.0, sh - h - 90.0, w, h), "DevLogica Tools – sei in call?");
                self.ask_win = UiWin::new(el, gpu, attrs, true);
            }
            if want_edges && self.edges.is_empty() {
                let mons: Vec<MonitorHandle> = el.available_monitors().collect();
                self.edges = ui::make_edges(el, gpu, &mons);
            }
        }
        if !want_panel {
            self.call_win = None;
        }
        if !want_coffee {
            self.coffee_win = None;
        }
        if !want_ask {
            self.ask_win = None;
        }
        if !want_edges {
            self.edges.clear();
        }
        self.refresh_tray();
    }

    fn paint_ui(&mut self, el: &ActiveEventLoop, force: bool) {
        let Some(gpu) = self.gpu.as_ref() else { return };
        let now = Instant::now();
        let old_name = self.me();
        let mut actions = Vec::new();
        let peers: Vec<String> = self.office.peers.keys().cloned().collect();
        let me = self.me();
        let update = self.update.as_ref().map(|u| u.0.clone());
        let ask = self.ask.clone();
        let mut view = View {
            cfg: &mut self.cfg,
            office: &self.office,
            me,
            mic: &self.detector.users,
            ask: ask.as_deref(),
            update: update.as_deref(),
            peers,
            actions: Vec::new(),
        };
        for (win, kind) in [(&mut self.call_win, 0), (&mut self.coffee_win, 1), (&mut self.ask_win, 2), (&mut self.settings, 3)] {
            let Some(w) = win.as_mut() else { continue };
            if !force && now < w.next_paint {
                continue;
            }
            match kind {
                0 => w.paint(gpu, ui::RED_BG, |u| ui::call_panel(u, &mut view)),
                1 => w.paint(gpu, ui::COFFEE_BG, |u| ui::coffee_panel(u, &mut view)),
                2 => w.paint(gpu, ui::INK, |u| ui::ask_panel(u, &mut view)),
                _ => w.paint(gpu, ui::INK, |u| ui::settings(u, &mut view)),
            }
        }
        actions.append(&mut view.actions);
        if !actions.is_empty() {
            self.handle_ui(el, actions, &old_name);
        }
    }

    // ------------------------------------------------------------------ menu

    fn refresh_tray(&mut self) {
        let badge = if !self.office.calls.is_empty() {
            Badge::Call
        } else if self.office.coffee.is_some() {
            Badge::Coffee
        } else {
            Badge::None
        };
        let sig = format!(
            "{:?}|{:?}|{:?}|{:?}|{}|{}|{:?}|{}|{}",
            self.office.calls.values().map(|c| (&c.user, c.mine)).collect::<Vec<_>>(),
            self.office.my_call.as_ref().map(|m| m.auto),
            self.office.coffee.as_ref().map(|c| (&c.by, c.mine)),
            self.update.as_ref().map(|u| &u.0),
            self.calls_hidden,
            self.coffee_hidden,
            serde_json::to_string(&self.cfg).unwrap_or_default(),
            self.wall.monitors.len(),
            self.catalog.len(),
        );
        if let Some(t) = &self.tray {
            if badge != self.badge {
                let _ = t.set_icon(Some(tray::icon(badge)));
                self.badge = badge;
            }
            if sig == self.menu_sig {
                return;
            }
        }
        self.menu_sig = sig;
        let built = tray::build(&MenuState {
            cfg: &self.cfg,
            office: &self.office,
            catalog: &self.catalog,
            monitors: &self.wall.monitors,
            autostart: platform::autostart_enabled(),
            update: self.update.as_ref().map(|u| u.0.as_str()),
            calls_hidden: self.calls_hidden,
            coffee_hidden: self.coffee_hidden,
        });
        self.actions = built.actions;
        let tooltip = match self.office.others().len() {
            0 => "DevLogica Tools".to_string(),
            1 => "DevLogica Tools – 1 call in corso".to_string(),
            n => format!("DevLogica Tools – {n} call in corso"),
        };
        match &self.tray {
            Some(t) => {
                t.set_menu(Some(Box::new(built.menu)));
                let _ = t.set_tooltip(Some(tooltip));
            }
            None => match TrayIconBuilder::new()
                .with_menu(Box::new(built.menu))
                .with_menu_on_left_click(true)
                .with_icon(tray::icon(badge))
                .with_tooltip(tooltip)
                .build()
            {
                Ok(t) => {
                    self.tray = Some(t);
                    self.badge = badge;
                }
                Err(e) => log(format!("Icona di sistema non creata: {e}")),
            },
        }
    }

    // ------------------------------------------------------------------ rilevamento

    fn poll_detector(&mut self, el: &ActiveEventLoop) {
        let settings_open = self.settings.is_some();
        match self.detector.poll(&self.cfg) {
            Some(Detect::Started(app)) => {
                if self.office.my_call.is_none() && !self.ask_declined {
                    match self.cfg.detection {
                        Detection::Auto => self.start_call(el, Some(app), true),
                        Detection::Ask => {
                            self.ask = Some(app);
                            sound::play(Chime::Call, self.cfg.volume * 0.5);
                            self.refresh(el);
                        }
                        Detection::Off => {}
                    }
                }
            }
            Some(Detect::Stopped) => {
                self.ask_declined = false;
                if self.ask.take().is_some() {
                    self.refresh(el);
                }
                if self.office.my_call.as_ref().map(|m| m.auto).unwrap_or(false) {
                    self.end_call(el);
                }
            }
            None => {}
        }
        // la finestra impostazioni mostra la diagnostica dal vivo
        if settings_open {
            if let Some(s) = self.settings.as_mut() {
                s.next_paint = Instant::now();
            }
        }
    }
}

impl ApplicationHandler<UserEvent> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        log(format!("Avvio DevLogica Tools {}, {} sfondi disponibili", env!("CARGO_PKG_VERSION"), self.catalog.len()));
        if !self.catalog.iter().any(|w| w.id == self.cfg.wallpaper) {
            self.cfg.wallpaper = self.catalog[0].id.clone();
        }
        self.wall.rebuild(el, &mut self.gpu, &self.cfg);
        if self.gpu.is_none() {
            log("Nessuna GPU disponibile: esco");
            el.exit();
            return;
        }

        // rete
        let proxy = self.proxy.clone();
        self.net = Net::start(move |p, from| {
            let _ = proxy.send_event(UserEvent::Packet(p, from));
        });
        let me = self.me();
        self.send_all(&Packet::Ping { user: Some(me) });

        // aggiornamenti: subito dopo l'avvio e poi due volte al giorno
        if update::repo().is_some() {
            let proxy = self.proxy.clone();
            let _ = std::thread::Builder::new().name("aggiornamenti".into()).spawn(move || loop {
                std::thread::sleep(Duration::from_secs(10));
                if let Some((tag, url)) = update::check() {
                    let _ = proxy.send_event(UserEvent::Update(tag, url));
                }
                std::thread::sleep(Duration::from_secs(12 * 3600));
            });
        }

        self.refresh(el);
        if self.cfg.name.trim().is_empty() {
            self.open_settings(el);
        }
    }

    fn user_event(&mut self, el: &ActiveEventLoop, event: UserEvent) {
        match event {
            UserEvent::Menu(id) => {
                if let Some(a) = self.actions.get(&id).cloned() {
                    self.handle_action(el, a);
                }
            }
            UserEvent::Packet(p, from) => self.on_packet(el, p, from),
            UserEvent::Update(tag, url) => {
                self.update = Some((tag, url));
                self.refresh_tray();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.wall.owns(id) {
            self.wall.window_event(self.gpu.as_ref(), &self.cfg, id, &event);
            return;
        }
        let Some(gpu) = self.gpu.as_ref() else { return };
        let is = |w: &Option<UiWin>| w.as_ref().map(|w| w.window.id() == id).unwrap_or(false);
        if is(&self.settings) && matches!(event, WindowEvent::CloseRequested) {
            self.settings = None;
            return;
        }
        for w in [&mut self.settings, &mut self.call_win, &mut self.coffee_win, &mut self.ask_win].into_iter().flatten() {
            if w.window.id() == id {
                w.on_event(gpu, &event);
                if matches!(event, WindowEvent::RedrawRequested) {
                    w.next_paint = Instant::now();
                }
            }
        }
        let _ = el;
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();

        if now >= self.next_check {
            self.next_check = now + Duration::from_secs(2);
            if self.wall.monitors_changed(el) {
                log("Configurazione schermi cambiata: ricreo le finestre");
                self.wall.rebuild(el, &mut self.gpu, &self.cfg);
                self.edges.clear();
                self.call_win = None;
                self.coffee_win = None;
                self.ask_win = None;
                self.refresh(el);
            }
            self.fullscreen = self.cfg.pause_on_fullscreen && platform::fullscreen_busy();
            let me = self.me();
            let (changed, out) = self.office.tick(&me);
            for p in out {
                self.send_all(&p);
            }
            if changed {
                self.refresh(el);
            }
        }

        if now >= self.next_detect {
            self.next_detect = now + Duration::from_millis(1500);
            self.poll_detector(el);
        }

        if now >= self.next_alive {
            self.next_alive = now + Duration::from_secs(20);
            let me = self.me();
            if let Some(p) = self.office.alive_packet(&me) {
                self.send_all(&p);
            }
        }

        // sfondi
        let wall_on = self.cfg.wallpaper_enabled && !self.cfg.paused && !self.fullscreen;
        if wall_on && now >= self.next_frame {
            if let Some(gpu) = self.gpu.as_mut() {
                self.wall.render(gpu, &self.cfg, &self.catalog);
            }
            let step = Duration::from_secs_f64(1.0 / self.cfg.fps() as f64);
            self.next_frame += step;
            if self.next_frame < now {
                self.next_frame = now + step;
            }
        }

        // bordo rosso pulsante
        if !self.edges.is_empty() && now >= self.next_edge {
            self.next_edge = now + Duration::from_millis(50);
            if let Some(gpu) = self.gpu.as_ref() {
                let c = ui::edge_color(self.started.elapsed().as_secs_f64());
                for e in &mut self.edges {
                    gpu.clear(&mut e.target, c);
                }
            }
        }

        self.paint_ui(el, false);

        // prossimo risveglio
        let mut wake = self.next_check.min(self.next_detect).min(self.next_alive);
        if wall_on {
            wake = wake.min(self.next_frame);
        }
        if !self.edges.is_empty() {
            wake = wake.min(self.next_edge);
        }
        for w in [&self.settings, &self.call_win, &self.coffee_win, &self.ask_win].into_iter().flatten() {
            wake = wake.min(w.next_paint);
        }
        el.set_control_flow(ControlFlow::WaitUntil(wake.max(Instant::now() + Duration::from_millis(5))));
    }
}
