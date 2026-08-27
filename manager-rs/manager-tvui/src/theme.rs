//! TASK-012g — `theme.json` şeması + yükleyici.
//!
//! `display/colors.py` (THEME_COLORS + BACKGROUND_THEME_PRESETS) + `fonts.py` +
//! `transitions.py` + `icons.py` tek bir SDL2 native tema şemasına portlanır
//! (EmulationStation `theme.xml` yerine serde_json ile tip-güvenli yükleme).

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Fonts {
    #[serde(default)]
    pub family: String,
    #[serde(default)]
    pub pixel_ttf: String,
    #[serde(default)]
    pub fallback: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BgPreset {
    pub top: Vec<u8>,
    pub bottom: Vec<u8>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Transitions {
    #[serde(default)]
    pub platform_select: Option<TransitionSpec>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TransitionSpec {
    #[serde(default)]
    pub duration_ms: u64,
    #[serde(default)]
    pub scale_min: f32,
    #[serde(default)]
    pub scale_max: f32,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Icons {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub set: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Theme {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub colors: HashMap<String, Vec<u8>>,
    #[serde(default)]
    pub background_presets: HashMap<String, BgPreset>,
    #[serde(default)]
    pub fonts: Fonts,
    #[serde(default)]
    pub transitions: Transitions,
    #[serde(default)]
    pub icons: Icons,
}

impl Theme {
    /// Renk adını `(r,g,b,a)`'ya çözer. 3-tuple alfa=255, 4-tuple alfa kullanılır.
    pub fn color(&self, name: &str) -> (u8, u8, u8, u8) {
        let v = self.colors.get(name).cloned().unwrap_or_default();
        match v.len() {
            4 => (v[0], v[1], v[2], v[3]),
            3 => (v[0], v[1], v[2], 255),
            2 => (v[0], v[1], v[1], 255),
            1 => (v[0], v[0], v[0], 255),
            _ => (255, 255, 255, 255),
        }
    }

    /// Seçili arka plan preset'inin `(top, bottom)` BGR'sini döndürür; yoksa default.
    pub fn background(&self, preset: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
        let p = self
            .background_presets
            .get(preset)
            .or_else(|| self.background_presets.get("default"));
        match p {
            Some(bg) => (tri(&bg.top), tri(&bg.bottom)),
            None => ((20, 25, 35), (45, 55, 75)),
        }
    }

    /// Dosyadan yükler; başarısızsa `Err`.
    pub fn load(path: &Path) -> Result<Theme, String> {
        let txt = std::fs::read_to_string(path)
            .map_err(|e| format!("tema dosyası okunamadı ({}): {e}", path.display()))?;
        let t: Theme =
            serde_json::from_str(&txt).map_err(|e| format!("tema JSON parse hatası: {e}"))?;
        Ok(t)
    }

    /// `fonts.pixel_ttf` dosyasının çözülmüş yolu (Faz 2: tema-relative).
    pub fn ttf_path(&self) -> std::path::PathBuf {
        crate::i18n::resolve_font_path(&self.fonts.pixel_ttf, std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
    }
}

fn tri(v: &[u8]) -> (u8, u8, u8) {
    match v.len() {
        3 | 4 => (v[0], v[1], v[2]),
        _ => (0, 0, 0),
    }
}

/// Gömülü varsayılan tema (`assets/theme.json`), fallback + unit test için.
pub fn default_theme() -> Theme {
    let txt = include_str!("../assets/theme.json");
    serde_json::from_str(txt).expect("gömülü varsayılan tema geçerli olmalı")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_embedded_default_theme() {
        let t = default_theme();
        assert_eq!(t.name, "default");
        assert!(t.colors.contains_key("fond_lignes"));
        assert_eq!(t.color("fond_lignes"), (0, 255, 0, 255));
        let (top, bottom) = t.background("default");
        assert_eq!(top, (20, 25, 35));
        assert_eq!(bottom, (45, 55, 75));
    }

    #[test]
    fn color_alpha_defaults_to_255_for_3tuple_and_kept_for_4tuple() {
        let t = default_theme();
        assert_eq!(t.color("text"), (255, 255, 255, 255));
        assert_eq!(t.color("button_idle"), (45, 50, 65, 180));
    }

    #[test]
    fn background_falls_back_to_default() {
        let t = default_theme();
        assert_eq!(t.background("nonexistent").0, (20, 25, 35));
    }

    #[test]
    fn all_background_presets_present() {
        let t = default_theme();
        for key in ["default", "sunset", "forest", "midnight"] {
            assert!(t.background_presets.contains_key(key), "preset yok: {key}");
        }
    }
}
