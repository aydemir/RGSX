//! Faz 12b (yön B) — TVUI native shell (rust-sdl2).
//!
//! `ports/RGSX/tvui.py` + `display/*` + `controls/*` (pygame native; arşiv:
//! python-skeleton-final tag'i) Rust'a birebir SDL2 primitives ile portlandı;
//! temalar `theme.json` ile serde_json yüklenir (ES `theme.xml` yerine).
//! `RGSX_TVUI=1` varsayılan (TASK-012l cutover) — `RGSX_TVUI=0` ile kapatılır;
//! tek TVUI yolu = native SDL2 (Python pygame fallback TASK-012-gap-02 ile söküldü).

pub mod accessibility;
pub mod folder_browser;
pub mod i18n;
pub mod menus;
pub mod native_input;
pub mod net;
pub mod render;
pub mod sdl2_shell;
pub mod state;
pub mod theme;
pub mod virtual_keyboard;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::theme::Theme;

/// TASK-012l cutover: `RGSX_TVUI` varsayılan 1, `0`/`false`/`off` ile kapatılır.
pub fn is_tvui_enabled() -> bool {
    match std::env::var("RGSX_TVUI") {
        Ok(v) => {
            let s = v.trim().to_ascii_lowercase();
            !(s == "0" || s == "false" || s == "off" || s.is_empty())
        }
        Err(_) => true,
    }
}

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
/// (bloklayıcı — ayrı thread'de çalışır). `port` manager-http portudur; SSE
/// `catalog_update` akışı bu porttan dinlenir (loading bar kaynağı).
///
/// Not (TASK-012-gap-01, bulgu 16): SDL video subsystem'i bazı platformlarda
/// (macOS) main thread ister. Hedefler Linux/Batocera + Windows'tur; macOS
/// hedeflenirse `launch`'ın main-thread'e taşınması gerekir.
#[cfg(test)]
mod tvui_flag_tests {
    use super::is_tvui_enabled;
    #[test]
    fn default_is_enabled() {
        std::env::remove_var("RGSX_TVUI");
        assert!(is_tvui_enabled());
    }
    #[test]
    fn explicit_off_disables() {
        std::env::set_var("RGSX_TVUI", "0");
        assert!(!is_tvui_enabled());
        std::env::set_var("RGSX_TVUI", "false");
        assert!(!is_tvui_enabled());
        std::env::remove_var("RGSX_TVUI");
    }
    #[test]
    fn explicit_on_enables() {
        std::env::set_var("RGSX_TVUI", "1");
        assert!(is_tvui_enabled());
        std::env::set_var("RGSX_TVUI", "true");
        assert!(is_tvui_enabled());
        std::env::remove_var("RGSX_TVUI");
    }
}

pub fn launch(_port: u16) -> Result<(), String> {
    let theme = load_theme();
    let state = crate::net::TvuiState::default();
    let shared = std::sync::Arc::new(std::sync::Mutex::new(state));
    // Shell çıkış bayrağı: gamepad `back` SSE'den gelir, SDL döngüsü okur;
    // watcher da aynı bayrakla temiz biter (sızan sonsuz reconnect yok).
    let shutdown = Arc::new(AtomicBool::new(false));
    let watcher = shared.clone();
    let flag = Arc::clone(&shutdown);
    let port = _port;
    // Arka plan SSE izleyici: katalog indirme ilerlemesini `shared`'a yazar.
    std::thread::spawn(move || {
        crate::net::start_catalog_watcher(port, watcher, &flag);
    });
    sdl2_shell::run_native_shell(&theme, &shared, &shutdown)
}
