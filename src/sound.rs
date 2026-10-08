//! Suono di avviso: due note morbide come in Call Alert. Generato al volo (nessun file audio)
//! e riprodotto con le funzioni del sistema, senza librerie audio aggiuntive.

use crate::config::app_dir;

#[derive(Clone, Copy)]
pub enum Chime {
    Call,
    Coffee,
}

/// WAV mono 16 bit: (frequenza, inizio, durata, volume) come nel Call Alert originale.
fn wav(notes: &[(f32, f32, f32, f32)], volume: f32) -> Vec<u8> {
    let rate = 44_100u32;
    let total = notes.iter().map(|n| n.1 + n.2).fold(0.0, f32::max) + 0.05;
    let n = (total * rate as f32) as usize;
    let mut s = vec![0f32; n];
    for &(freq, start, dur, peak) in notes {
        let a = (start * rate as f32) as usize;
        let len = (dur * rate as f32) as usize;
        for i in 0..len.min(n.saturating_sub(a)) {
            let t = i as f32 / rate as f32;
            // attacco di 40 ms, poi decadimento esponenziale
            let env = if t < 0.04 { t / 0.04 } else { (0.001f32).powf((t - 0.04) / (dur - 0.04)) };
            s[a + i] += (2.0 * std::f32::consts::PI * freq * t).sin() * env * peak * volume;
        }
    }
    let data: Vec<u8> = s.iter().flat_map(|v| ((v.clamp(-1.0, 1.0) * 32_000.0) as i16).to_le_bytes()).collect();
    let mut out = Vec::with_capacity(44 + data.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&rate.to_le_bytes());
    out.extend_from_slice(&(rate * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&data);
    out
}

fn notes(c: Chime) -> &'static [(f32, f32, f32, f32)] {
    match c {
        Chime::Call => &[(523.0, 0.0, 0.8, 0.18), (659.0, 0.22, 0.9, 0.13)],
        Chime::Coffee => &[(659.0, 0.0, 0.6, 0.15), (784.0, 0.16, 0.6, 0.13), (988.0, 0.32, 0.9, 0.11)],
    }
}

pub fn play(c: Chime, volume: f32) {
    if volume <= 0.01 {
        return;
    }
    // Il file viene riscritto solo se cambia il volume.
    let name = match c {
        Chime::Call => "call",
        Chime::Coffee => "caffe",
    };
    let path = app_dir().join(format!("suono-{name}-{}.wav", (volume * 100.0).round() as u32));
    if !path.exists() {
        let _ = std::fs::create_dir_all(app_dir());
        let _ = std::fs::write(&path, wav(notes(c), volume));
    }
    #[cfg(windows)]
    {
        use windows::core::HSTRING;
        use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};
        let p = HSTRING::from(path.as_os_str());
        unsafe {
            let _ = PlaySoundW(&p, None, SND_FILENAME | SND_ASYNC | SND_NODEFAULT);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay").arg(&path).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = std::process::Command::new("paplay").arg(&path).spawn();
    }
}
