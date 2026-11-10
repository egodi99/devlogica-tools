//! Notifiche di sistema. Il clic su una notifica di aggiornamento fa partire l'aggiornamento.
//! - macOS: UNUserNotificationCenter con un delegate; se non è disponibile (app non in un
//!   bundle, permesso negato) si ripiega su `osascript`, che però non riporta il clic.
//! - Windows: balloon dell'area di notifica (vedi `platform::windows::show_balloon`).

use crate::config::log;
use std::sync::OnceLock;

const TITLE: &str = "DevLogica Tools";

static ON_CLICK: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

/// Registra cosa fare quando si clicca una notifica di aggiornamento. Va chiamata una volta,
/// sul thread principale, all'avvio.
pub fn init(on_click: impl Fn() + Send + Sync + 'static) {
    let _ = ON_CLICK.set(Box::new(on_click));
    imp::init();
}

/// Chiamata dai backend quando l'utente clicca una notifica di aggiornamento.
#[allow(dead_code)]
pub(crate) fn clicked() {
    log("Notifica di aggiornamento cliccata");
    if let Some(f) = ON_CLICK.get() {
        f();
    }
}

/// Vero se su questo sistema il clic sulla notifica arriva davvero all'app.
pub fn clickable() -> bool {
    imp::clickable()
}

/// Mostra una notifica con titolo "DevLogica Tools". Con `click_updates` il clic avvia l'aggiornamento.
pub fn show(body: &str, click_updates: bool) {
    log(format!("Notifica: {body}"));
    imp::show(TITLE, body, click_updates);
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2::rc::Retained;
    use objc2::runtime::ProtocolObject;
    use objc2::{define_class, msg_send, AllocAnyThread};
    use objc2_foundation::{NSBundle, NSError, NSObject, NSObjectProtocol, NSString};
    use objc2_user_notifications::{
        UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationPresentationOptions, UNNotificationRequest,
        UNNotificationResponse, UNUserNotificationCenter, UNUserNotificationCenterDelegate,
    };
    use std::sync::atomic::{AtomicU8, Ordering};

    /// 0 = non ancora noto, 1 = consentite, 2 = non disponibili o negate.
    static AUTH: AtomicU8 = AtomicU8::new(0);

    define_class!(
        #[unsafe(super(NSObject))]
        #[name = "DLTNotificheDelegate"]
        struct Delegate;

        unsafe impl NSObjectProtocol for Delegate {}

        unsafe impl UNUserNotificationCenterDelegate for Delegate {
            #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
            fn did_receive(&self, _center: &UNUserNotificationCenter, response: &UNNotificationResponse, done: &block2::DynBlock<dyn Fn()>) {
                let id = response.notification().request().identifier().to_string();
                if id.starts_with("aggiorna") {
                    super::clicked();
                }
                done.call(());
            }

            #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
            fn will_present(&self, _center: &UNUserNotificationCenter, _n: &UNNotification, done: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>) {
                done.call((UNNotificationPresentationOptions::Banner | UNNotificationPresentationOptions::List,));
            }
        }
    );

    /// UNUserNotificationCenter funziona solo dentro un'app (.app con bundle id).
    fn in_bundle() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    pub fn init() {
        if !in_bundle() {
            AUTH.store(2, Ordering::SeqCst);
            return;
        }
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let delegate: Retained<Delegate> = unsafe { msg_send![Delegate::alloc(), init] };
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // il centro tiene un riferimento debole: il delegate deve vivere per sempre
        std::mem::forget(delegate);
        let block = block2::RcBlock::new(|granted: objc2::runtime::Bool, _err: *mut NSError| {
            AUTH.store(if granted.as_bool() { 1 } else { 2 }, Ordering::SeqCst);
            crate::config::log(format!("Notifiche di sistema: {}", if granted.as_bool() { "consentite" } else { "non consentite" }));
        });
        center.requestAuthorizationWithOptions_completionHandler(UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound, &block);
    }

    pub fn clickable() -> bool {
        AUTH.load(Ordering::SeqCst) == 1
    }

    pub fn show(title: &str, body: &str, click_updates: bool) {
        // Con la firma ad-hoc macOS spesso non autorizza le notifiche: in quel caso (o finché
        // non ha risposto) si usa AppleScript, così la notifica compare comunque, senza clic.
        if AUTH.load(Ordering::SeqCst) != 1 {
            fallback(title, body);
            return;
        }
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        let prefix = if click_updates { "aggiorna" } else { "info" };
        let id = NSString::from_str(&format!("{prefix}-{}", crate::config::now_ms() as u64));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(&id, &content, None);
        let center = UNUserNotificationCenter::currentNotificationCenter();
        center.addNotificationRequest_withCompletionHandler(&request, None);
    }

    /// Notifica senza clic, per quando UNUserNotificationCenter non è disponibile.
    fn fallback(title: &str, body: &str) {
        let esc = |s: &str| s.replace('\\', "\\\\").replace('"', "\\\"");
        let script = format!("display notification \"{}\" with title \"{}\"", esc(body), esc(title));
        let _ = std::process::Command::new("osascript").args(["-e", &script]).spawn();
    }
}

#[cfg(windows)]
mod imp {
    pub fn init() {}
    pub fn clickable() -> bool {
        true
    }
    pub fn show(title: &str, body: &str, _click_updates: bool) {
        crate::platform::show_balloon(title, body);
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn init() {}
    pub fn clickable() -> bool {
        false
    }
    pub fn show(_title: &str, _body: &str, _click_updates: bool) {}
}

/// Prova delle notifiche dalla riga di comando: `--notifica-prova`. Mostra una notifica
/// cliccabile e resta in attesa del clic per 60 secondi (va lanciata dall'app .app su macOS).
pub fn run_test() -> i32 {
    init(|| {
        println!("CLIC RICEVUTO");
        crate::config::log("Prova notifiche: clic ricevuto");
        std::process::exit(0);
    });
    let wait = |secs: f64| {
        #[cfg(target_os = "macos")]
        unsafe {
            #[link(name = "CoreFoundation", kind = "framework")]
            unsafe extern "C" {
                static kCFRunLoopDefaultMode: *const std::ffi::c_void;
                fn CFRunLoopRunInMode(mode: *const std::ffi::c_void, secs: f64, ret: u8) -> i32;
            }
            CFRunLoopRunInMode(kCFRunLoopDefaultMode, secs, 0);
        }
        #[cfg(not(target_os = "macos"))]
        std::thread::sleep(std::time::Duration::from_secs_f64(secs));
    };
    wait(3.0); // tempo per la richiesta di permesso
    let msg = if clickable() { "Prova: clicca questa notifica." } else { "Prova: notifica senza clic." };
    println!("cliccabile = {}", clickable());
    crate::config::log(format!("Prova notifiche: cliccabile = {}", clickable()));
    show(msg, true);
    wait(60.0);
    println!("nessun clic");
    1
}
