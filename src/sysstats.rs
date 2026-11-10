//! Statistiche di sistema per gli indicatori: CPU complessiva e RAM usata.
//! I calcoli sono puri e testati; la lettura vera avviene in `platform` (solo macOS),
//! in un thread che campiona ogni 2 secondi.

use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stats {
    /// Percentuale complessiva di tutti i core, 0..100.
    pub cpu: f32,
    pub ram_used: u64,
    pub ram_total: u64,
}

/// CPU usata tra due letture dei tick `[user, system, idle, nice]`.
/// Contatori fermi o tornati indietro danno 0, mai NaN.
pub fn cpu_percent(prev: [u64; 4], cur: [u64; 4]) -> f32 {
    if cur.iter().zip(prev.iter()).any(|(c, p)| c < p) {
        return 0.0;
    }
    let d: Vec<u64> = cur.iter().zip(prev.iter()).map(|(c, p)| c - p).collect();
    let total: u64 = d.iter().sum();
    if total == 0 {
        return 0.0;
    }
    let busy = d[0] + d[1] + d[3];
    (busy as f64 / total as f64 * 100.0) as f32
}

/// "Memoria utilizzata" come in Monitoraggio Attività: memoria app + wired + compressa.
pub fn ram_used(internal: u64, purgeable: u64, wired: u64, compressed: u64, page: u64) -> u64 {
    (internal.saturating_sub(purgeable) + wired + compressed) * page
}

/// Campiona CPU e RAM ogni 2 secondi in un thread a parte; il disegno legge solo l'ultimo valore.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub struct Sampler {
    latest: Arc<Mutex<Stats>>,
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
impl Sampler {
    pub fn start() -> Self {
        use crate::platform;
        let latest = Arc::new(Mutex::new(Stats::default()));
        let out = latest.clone();
        let _ = std::thread::Builder::new().name("statistiche".into()).spawn(move || {
            let mut prev: Option<[u64; 4]> = None;
            loop {
                let ticks = platform::cpu_ticks();
                let cpu = match (prev, ticks) {
                    (Some(p), Some(c)) => cpu_percent(p, c),
                    _ => 0.0,
                };
                prev = ticks;
                let ram_total = platform::mem_total().unwrap_or(0);
                let used = platform::vm_pages().map(|(i, p, w, c, pg)| ram_used(i, p, w, c, pg)).unwrap_or(0);
                if let Ok(mut s) = out.lock() {
                    *s = Stats { cpu, ram_used: used.min(ram_total), ram_total };
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
        Self { latest }
    }

    pub fn latest(&self) -> Stats {
        self.latest.lock().map(|s| *s).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_percent_base() {
        assert_eq!(cpu_percent([100, 50, 850, 0], [130, 70, 1000, 0]), 25.0);
    }

    #[test]
    fn cpu_percent_nessun_delta() {
        assert_eq!(cpu_percent([1, 2, 3, 4], [1, 2, 3, 4]), 0.0);
        let back = cpu_percent([100, 100, 100, 100], [50, 50, 50, 50]);
        assert_eq!(back, 0.0);
    }

    #[test]
    fn ram_used_base() {
        assert_eq!(ram_used(1000, 100, 200, 50, 16384), (1000 - 100 + 200 + 50) * 16384);
        assert_eq!(ram_used(10, 100, 0, 0, 4096), 0);
    }
}

