//! Catalogo degli sfondi: quelli inclusi nell'eseguibile più eventuali shader
//! personalizzati (*.wgsl) nella cartella "sfondi" della configurazione.

use crate::config;

pub const PRELUDE: &str = include_str!("../shaders/prelude.wgsl");

#[derive(Clone, Debug)]
pub struct Wallpaper {
    pub id: String,
    pub name: String,
    pub source: String,
    /// Sfondo fisso: si disegna una volta sola e si ridisegna solo quando serve.
    pub still: bool,
}

/// Uno sfondo è fisso se nell'intestazione (le prime righe di commento) c'è "// tipo: fisso".
pub fn is_still(source: &str) -> bool {
    source
        .lines()
        .take_while(|l| l.trim_start().starts_with("//"))
        .filter_map(|l| l.trim_start().strip_prefix("//").and_then(|r| r.trim().strip_prefix("tipo:")))
        .any(|v| v.trim().eq_ignore_ascii_case("fisso"))
}

const BUILTIN: &[(&str, &str, &str)] = &[
    ("lame", "Lame di luce", include_str!("../shaders/lame.wgsl")),
    ("barre", "Barre diagonali", include_str!("../shaders/barre.wgsl")),
    ("geometrie", "Geometrie", include_str!("../shaders/geometrie.wgsl")),
    ("reticolo", "Reticolo", include_str!("../shaders/reticolo.wgsl")),
    ("cruscotto", "Cruscotto", include_str!("../shaders/cruscotto.wgsl")),
    ("costellazione", "Costellazione", include_str!("../shaders/costellazione.wgsl")),
    ("flusso", "Flusso", include_str!("../shaders/flusso.wgsl")),
    ("aurora", "Aurora", include_str!("../shaders/aurora.wgsl")),
    // fissi
    ("quiete", "Quiete", include_str!("../shaders/quiete.wgsl")),
    ("prisma", "Prisma", include_str!("../shaders/prisma.wgsl")),
    ("isoipse", "Isoipse", include_str!("../shaders/isoipse.wgsl")),
];

pub fn catalog() -> Vec<Wallpaper> {
    let mut list: Vec<Wallpaper> = BUILTIN
        .iter()
        .map(|(id, name, src)| Wallpaper { id: (*id).into(), name: (*name).into(), source: (*src).into(), still: is_still(src) })
        .collect();

    let dir = config::custom_wallpapers_dir();
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut files: Vec<_> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e.eq_ignore_ascii_case("wgsl")).unwrap_or(false))
            .collect();
        files.sort();
        for path in files {
            let Ok(source) = std::fs::read_to_string(&path) else { continue };
            let stem = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            // Il nome visualizzato si può indicare nella prima riga: "// nome: Il mio sfondo"
            let name = source
                .lines()
                .next()
                .and_then(|l| l.trim().strip_prefix("//"))
                .and_then(|l| l.trim().strip_prefix("nome:"))
                .map(|n| n.trim().to_string())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| stem.clone());
            let still = is_still(&source);
            list.push(Wallpaper { id: format!("custom:{stem}"), name, source, still });
        }
    }
    list
}

pub fn full_source(w: &Wallpaper) -> String {
    // il cruscotto riceve le posizioni degli strumenti da dash.rs
    let extra = if w.id == crate::dash::ID { crate::dash::wgsl_consts() } else { String::new() };
    format!("{PRELUDE}\n{extra}\n// ---- {} ----\n{}", w.name, w.source)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn riconosce_gli_sfondi_fissi() {
        assert!(is_still("// nome: Prova\n// tipo: fisso\nfn scene() {}"));
        assert!(is_still("// tipo:Fisso\n"));
        assert!(!is_still("// nome: Prova\nfn scene() {}"));
        // conta solo l'intestazione, non un commento più in basso
        assert!(!is_still("// nome: Prova\nfn scene() {}\n// tipo: fisso"));
    }

    #[test]
    fn catalogo_con_animati_e_fissi() {
        let cat = catalog();
        assert!(cat.iter().any(|w| w.still));
        assert!(cat.iter().any(|w| !w.still));
        // il primo è quello di ripiego: deve essere animato
        assert!(!cat[0].still);
    }
}
