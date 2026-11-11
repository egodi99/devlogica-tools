//! Letture per il cruscotto su Windows: CPU, RAM, GPU, disco, rete, memoria di WSL,
//! spazio su C:, batteria e tempo di accensione. GPU, disco, rete e WSL arrivano dai
//! contatori di prestazioni (PDH), gli stessi di Gestione attività: nessun permesso speciale.

use crate::dash::DashStats;
use windows::core::w;
use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Performance::*;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GetTickCount64, GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

pub struct Probe {
    query: Option<PDH_HQUERY>,
    gpu: Option<PDH_HCOUNTER>,
    disk: Option<PDH_HCOUNTER>,
    net: Option<PDH_HCOUNTER>,
    wsl: Option<PDH_HCOUNTER>,
    prev_cpu: Option<(u64, u64, u64)>,
}

fn ft(f: FILETIME) -> u64 {
    (f.dwHighDateTime as u64) << 32 | f.dwLowDateTime as u64
}

impl Probe {
    pub fn new() -> Self {
        let mut p = Probe { query: None, gpu: None, disk: None, net: None, wsl: None, prev_cpu: None };
        unsafe {
            let mut q = PDH_HQUERY::default();
            if PdhOpenQueryW(windows::core::PCWSTR::null(), 0, &mut q) != 0 {
                crate::config::log("Cruscotto: contatori di prestazioni non disponibili");
                return p;
            }
            let add = |path: windows::core::PCWSTR| {
                let mut c = PDH_HCOUNTER::default();
                (PdhAddEnglishCounterW(q, path, 0, &mut c) == 0).then_some(c)
            };
            p.gpu = add(w!("\\GPU Engine(*engtype_3D)\\Utilization Percentage"));
            p.disk = add(w!("\\PhysicalDisk(_Total)\\% Idle Time"));
            p.net = add(w!("\\Network Interface(*)\\Bytes Total/sec"));
            // vmmemWSL (Windows 11) o vmmem (Windows 10): la macchina virtuale di WSL 2 e Docker
            p.wsl = add(w!("\\Process(vmmem*)\\Working Set"));
            let _ = PdhCollectQueryData(q);
            p.query = Some(q);
        }
        p
    }

    /// Valori (nome istanza, valore) di un contatore con più istanze.
    fn items(c: Option<PDH_HCOUNTER>) -> Option<Vec<(String, f64)>> {
        let c = c?;
        unsafe {
            let (mut size, mut count) = (0u32, 0u32);
            let r = PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, None);
            if r != PDH_MORE_DATA || size == 0 {
                return None;
            }
            // buffer allineato come gli elementi (contengono puntatori e double)
            let mut buf: Vec<u64> = vec![0; (size as usize).div_ceil(8)];
            let items = buf.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W;
            if PdhGetFormattedCounterArrayW(c, PDH_FMT_DOUBLE, &mut size, &mut count, Some(items)) != 0 {
                return None;
            }
            let list = std::slice::from_raw_parts(items, count as usize);
            let out: Vec<(String, f64)> = list
                .iter()
                .filter(|it| it.FmtValue.CStatus == PDH_CSTATUS_VALID_DATA || it.FmtValue.CStatus == 1)
                .map(|it| (it.szName.to_string().unwrap_or_default(), it.FmtValue.Anonymous.doubleValue))
                .collect();
            (!out.is_empty()).then_some(out)
        }
    }

    /// Somma dei valori di un contatore con più istanze (`None` se non ce n'è nessuna).
    fn sum(c: Option<PDH_HCOUNTER>) -> Option<f64> {
        Self::items(c).map(|v| v.iter().map(|(_, x)| x).sum())
    }

    /// GPU come in Gestione attività: per ogni motore la somma dei processi, poi il più occupato.
    fn gpu_busy(c: Option<PDH_HCOUNTER>) -> Option<f64> {
        Self::items(c).map(|v| crate::dash::gpu_from_engines(&v))
    }

    fn single(c: Option<PDH_HCOUNTER>) -> Option<f64> {
        let c = c?;
        unsafe {
            let mut v = PDH_FMT_COUNTERVALUE::default();
            (PdhGetFormattedCounterValue(c, PDH_FMT_DOUBLE, None, &mut v) == 0).then(|| v.Anonymous.doubleValue)
        }
    }

    pub fn sample(&mut self) -> DashStats {
        let mut s = DashStats { ready: true, ..Default::default() };
        unsafe {
            // CPU: tempi totali di sistema tra due letture
            let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                let cur = (ft(idle), ft(kernel), ft(user));
                if let Some(p) = self.prev_cpu {
                    let (di, dk, du) = (cur.0.saturating_sub(p.0), cur.1.saturating_sub(p.1), cur.2.saturating_sub(p.2));
                    let total = dk + du; // il tempo del kernel comprende quello inattivo
                    if total > 0 {
                        s.cpu = ((total.saturating_sub(di)) as f64 / total as f64 * 100.0) as f32;
                    }
                }
                self.prev_cpu = Some(cur);
            }
            let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
            if GlobalMemoryStatusEx(&mut m).is_ok() && m.ullTotalPhys > 0 {
                s.ram = ((m.ullTotalPhys - m.ullAvailPhys) as f64 / m.ullTotalPhys as f64 * 100.0) as f32;
                s.ram_total_gb = (m.ullTotalPhys as f64 / 1_073_741_824.0) as f32;
            }
            s.uptime_h = (GetTickCount64() as f64 / 3_600_000.0) as f32;
            let mut ps = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut ps).is_ok() && ps.BatteryFlag != 128 && ps.BatteryLifePercent <= 100 {
                s.battery = Some((ps.BatteryLifePercent as f32, ps.ACLineStatus == 1));
            }
            let (mut free, mut total) = (0u64, 0u64);
            if windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(w!("C:\\"), None, Some(&mut total), Some(&mut free)).is_ok() && total > 0 {
                s.disk_c = Some(((total - free) as f64 / total as f64 * 100.0) as f32);
            }
            if let Some(q) = self.query {
                let _ = PdhCollectQueryData(q);
            }
        }
        s.gpu = Self::gpu_busy(self.gpu).map(|v| v.clamp(0.0, 100.0) as f32);
        s.disk = Self::single(self.disk).map(|idle| (100.0 - idle).clamp(0.0, 100.0) as f32);
        s.net_mbit = Self::sum(self.net).map(|b| (b * 8.0 / 1_000_000.0) as f32);
        s.wsl_gb = Self::sum(self.wsl).filter(|b| *b > 0.0).map(|b| (b / 1_073_741_824.0) as f32);
        s
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        if let Some(q) = self.query.take() {
            unsafe {
                let _ = PdhCloseQuery(q);
            }
        }
    }
}

/// Container Docker in esecuzione, chiesti al demone sulla sua named pipe (come fa `docker ps`).
/// `None` se Docker non è avviato.
pub fn docker_running() -> Option<u32> {
    use std::io::{Read, Write};
    let mut pipe = std::fs::OpenOptions::new().read(true).write(true).open(r"\\.\pipe\docker_engine").ok()?;
    pipe.write_all(b"GET /containers/json HTTP/1.0\r\nHost: docker\r\n\r\n").ok()?;
    let mut out = Vec::new();
    pipe.read_to_end(&mut out).ok()?;
    crate::dash::count_containers(&out)
}
