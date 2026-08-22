//! Faz 12b (yön B) — TVUI native shell (rust-sdl2).
//!
//! `ports/RGSX/tvui.py` + `display/*` + `controls/*` (pygame native) Rust'a birebir
//! SDL2 primitives ile portlanır; temalar `theme.json` (`colors.py`+`fonts.py`+
//! `transitions.py`+`icons.py`) ile serde_json yüklenir (ES `theme.xml` yerine).
//! `RGSX_TVUI=1` → SDL2 native shell; `RGSX_TVUI=0` → eski Python pygame fallback
//! (bu crate o durumda çağrılmaz).

pub mod native_input;
pub mod sdl2_shell;
pub mod theme;

use std::path::PathBuf;

use crate::theme::Theme;

/// Tema yükler: `RGSX_TVUI_THEME` env → dosya; yoksa gömülü varsayılan (`theme.json`).
pub fn load_theme() -> Theme {
    if let Ok(p) = std::env::var("RGSX_TVUI_THEME") {
        let path = PathBuf::from(p.clone());
        if let Ok(t) = Theme::load(&path) {
            eprintln!("TVUI tema yüklendi (dosya): {}", path.display());
            return t;
        }
        eprintln!("TVUI tema dosyası okunamadı, gömülü varsayılana dönülüyor: {p}");
    }
    theme::default_theme()
}

/// `RGSX_TVUI=1` iken manager-bin tarafından çağrılır: SDL2 native shell'i açar
/// (bloklayıcı — ayrı thread'de çalışır). `port` ileride SSE bağlantısı için saklanır.
pub fn launch(_port: u16) -> Result<(), String> {
    let theme = load_theme();
    sdl2_shell::run_native_shell(&theme)
}
