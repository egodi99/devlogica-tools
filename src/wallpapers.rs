//! Catalogo degli sfondi: quelli inclusi nell'eseguibile più eventuali shader
//! personalizzati (*.wgsl) nella cartella "sfondi" della configurazione.

use crate::config;

pub const PRELUDE: &str = include_str!("../shaders/prelude.wgsl");

#[derive(Clone, Debug)]
pub struct Wallpaper {
    pub id: String,
    pub name: String,
    pub source: String,
}

const BUILTIN: &[(&str, &str, &str)] = &[
    ("lame", "Lame di luce", include_str!("../shaders/lame.wgsl")),
    ("barre", "Barre diagonali", include_str!("../shaders/barre.wgsl")),
    ("geometrie", "Geometrie", include_str!("../shaders/geometrie.wgsl")),
    ("reticolo", "Reticolo", include_str!("../shaders/reticolo.wgsl")),
];

pub fn catalog() -> Vec<Wallpaper> {
    let mut list: Vec<Wallpaper> = BUILTIN
        .iter()
        .map(|(id, name, src)| Wallpaper { id: (*id).into(), name: (*name).into(), source: (*src).into() })
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
            list.push(Wallpaper { id: format!("custom:{stem}"), name, source });
        }
    }
    list
}

pub fn full_source(w: &Wallpaper) -> String {
    format!("{PRELUDE}\n// ---- {} ----\n{}", w.name, w.source)
}
