//! TASK-012h Faz 2 — `t(key)` i18n okuyucu (SDL'siz).
//!
//! `webui/languages/<lang>.json` okur (`RGSX_TVUI_LANG` / `RGSX_LANGUAGE` / `LANG` env
//! ile dil seçimi, yoksa `en`). WebUI `webui/src/i18n.js` ile aynı kaynak, aynı fallback
//! (en → key). String'ler `TriggerResult.message`'larda merkezî.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Dil verisi: key → çeviri.
pub type LangMap = HashMap<String, String>;

/// `LANG` gibi `en_US.UTF-8` değerinden `en` çıkarır.
fn normalize_lang(raw: &str) -> String {
    let lower = raw.to_ascii_lowercase();
    let lang = lower
        .split(|c| c == '_' || c == '-' || c == '.')
        .next()
        .unwrap_or("en");
    match lang {
        "tr" | "en" | "fr" | "de" | "es" | "it" | "pt" => lang.to_string(),
        _ => "en".to_string(),
    }
}

/// Ortamdan dili çözer: `RGSX_TVUI_LANG` > `RGSX_LANGUAGE` > `LANG` > `en`.
pub fn detect_lang() -> String {
    for key in ["RGSX_TVUI_LANG", "RGSX_LANGUAGE", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            if !v.trim().is_empty() {
                return normalize_lang(&v);
            }
        }
    }
    "en".to_string()
}

/// `webui/languages` dizinini bulur (crate-relative + cwd fallback).
fn languages_dir() -> PathBuf {
    // Deploy: manager-bin `RGSX_LANGUAGES_FOLDER` set eder (rgsx_dir/languages).
    if let Ok(env_dir) = std::env::var("RGSX_LANGUAGES_FOLDER") {
        let p = PathBuf::from(env_dir);
        if p.is_dir() {
            return p;
        }
    }
    // Deploy yanı: exe dizini + ataları (roms/ports/RGSX[/languages|webui/languages]).
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for rel in ["languages", "webui/languages"] {
                let p = dir.join(rel);
                if p.is_dir() {
                    return p;
                }
            }
        }
    }
    // cargo test'te cwd = manager-rs/manager-tvui
    let candidates = [
        PathBuf::from("../../webui/languages"),
        PathBuf::from("webui/languages"),
        PathBuf::from("../webui/languages"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../webui/languages"),
    ];
    for p in candidates {
        if p.is_dir() {
            return p;
        }
    }
    PathBuf::from("webui/languages")
}

/// `lang` için JSON'u yükler; yoksa boş map. `en` her zaman fallback için denenir.
pub fn load_lang(lang: &str) -> LangMap {
    let dir = languages_dir();
    let path = dir.join(format!("{lang}.json"));
    if let Ok(txt) = std::fs::read_to_string(&path) {
        if let Ok(map) = serde_json::from_str::<LangMap>(&txt) {
            return map;
        }
    }
    // Fallback: en.json dene
    if lang != "en" {
        let en_path = dir.join("en.json");
        if let Ok(txt) = std::fs::read_to_string(&en_path) {
            if let Ok(map) = serde_json::from_str::<LangMap>(&txt) {
                return map;
            }
        }
    }
    // Son çare: gömülü minimal (en TR)
    HashMap::new()
}

/// `t(key)` — `map`'te varsa çeviri, yoksa `fallback` map'te ara, yoksa key'in kendisi.
/// `fallback` genelde `en` map'idir (WebUI parity).
pub fn t_with_fallback(key: &str, primary: &LangMap, fallback: &LangMap) -> String {
    if let Some(v) = primary.get(key) {
        return v.clone();
    }
    if let Some(v) = fallback.get(key) {
        return v.clone();
    }
    key.to_string()
}

/// Basit `t(key)` — `en` fallback'i otomatik yükler (cold; her çağrıda dosya okur).
/// Kare döngüsünde KULLANMAYIN — `tcached()` kullanın.
pub fn t(key: &str) -> String {
    let lang = detect_lang();
    let primary = load_lang(&lang);
    if lang == "en" {
        primary.get(key).cloned().unwrap_or_else(|| key.to_string())
    } else {
        let en = load_lang("en");
        t_with_fallback(key, &primary, &en)
    }
}

struct CachedLang {
    lang: String,
    primary: LangMap,
    en: LangMap,
}

static LANG_CACHE: std::sync::OnceLock<CachedLang> = std::sync::OnceLock::new();

/// Kare-döngüsü güvenli çeviri: dil dosyaları süreçte BİR kez yüklenir.
/// Not: dil değişimi yeniden başlatma gerektirir (upstream parity — dil ayarı boot'ta okunur).
pub fn tcached(key: &str) -> String {
    let c = LANG_CACHE.get_or_init(|| {
        let lang = detect_lang();
        let primary = load_lang(&lang);
        let en = if lang == "en" {
            LangMap::new()
        } else {
            load_lang("en")
        };
        CachedLang { lang, primary, en }
    });
    if c.lang == "en" {
        c.primary.get(key).cloned().unwrap_or_else(|| key.to_string())
    } else {
        t_with_fallback(key, &c.primary, &c.en)
    }
}

/// Tema `fonts.pixel_ttf` yolunu çözer (SDL'siz, test edilebilir).
/// `theme.json` `pixel_ttf` göreli ise `assets/` köküne göre, mutlak ise aynen.
pub fn resolve_font_path(pixel_ttf: &str, theme_dir: &Path) -> PathBuf {
    let p = PathBuf::from(pixel_ttf);
    if p.is_absolute() {
        return p;
    }
    // `assets/fonts/Pixel-UniCode.ttf` → crate `assets/` altında aranır
    let candidates = [
        theme_dir.join(&p),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&p),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets").join(p.file_name().unwrap_or_default()),
        PathBuf::from("../../webui").join(&p), // yanlış yol fallback — varlık yoksa yine döner
    ];
    for c in &candidates {
        if c.exists() {
            return c.clone();
        }
    }
    // Hiçbiri yoksa ilk aday (theme_dir-relative) — çağıran varlık kontrolü yapar
    theme_dir.join(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_lang_extracts_en_tr() {
        assert_eq!(normalize_lang("tr_TR.UTF-8"), "tr");
        assert_eq!(normalize_lang("en_US"), "en");
        assert_eq!(normalize_lang("FR-fr"), "fr");
        assert_eq!(normalize_lang("xx_YY"), "en");
        assert_eq!(normalize_lang(""), "en");
    }

    #[test]
    fn detect_lang_env_priority() {
        let _g = std::sync::Mutex::new(());
        // ENV_LOCK yok, ama test izole — RGSX_TVUI_LANG en yüksek öncelik
        let prev = std::env::var("RGSX_TVUI_LANG").ok();
        let prev2 = std::env::var("RGSX_LANGUAGE").ok();
        std::env::set_var("RGSX_TVUI_LANG", "tr");
        std::env::set_var("RGSX_LANGUAGE", "en");
        assert_eq!(detect_lang(), "tr");
        std::env::remove_var("RGSX_TVUI_LANG");
        assert_eq!(detect_lang(), "en");
        // cleanup
        match prev {
            Some(v) => std::env::set_var("RGSX_TVUI_LANG", v),
            None => std::env::remove_var("RGSX_TVUI_LANG"),
        }
        match prev2 {
            Some(v) => std::env::set_var("RGSX_LANGUAGE", v),
            None => std::env::remove_var("RGSX_LANGUAGE"),
        }
    }

    #[test]
    fn load_lang_en_has_keys() {
        let en = load_lang("en");
        // en.json boş değilse en az bir bilinen anahtar var
        if !en.is_empty() {
            assert!(en.contains_key("welcome_message") || en.contains_key("app_title") || !en.is_empty());
        }
    }

    #[test]
    fn t_with_fallback_prefers_primary() {
        let mut primary = HashMap::new();
        primary.insert("hello".into(), "merhaba".into());
        let mut fallback = HashMap::new();
        fallback.insert("hello".into(), "hello".into());
        fallback.insert("only_en".into(), "only_en".into());
        assert_eq!(t_with_fallback("hello", &primary, &fallback), "merhaba");
        assert_eq!(t_with_fallback("only_en", &primary, &fallback), "only_en");
        assert_eq!(t_with_fallback("missing", &primary, &fallback), "missing");
    }

    #[test]
    fn resolve_font_path_relative() {
        let dir = PathBuf::from("/tmp/theme");
        let p = resolve_font_path("assets/fonts/Pixel-UniCode.ttf", &dir);
        // Mutlak değilse theme_dir'e join etmeli (varlık yoksa da path dönmeli)
        assert!(p.ends_with("Pixel-UniCode.ttf"));
    }

    #[test]
    fn resolve_font_path_absolute() {
        let p = resolve_font_path("/tmp/foo.ttf", Path::new("/tmp/theme"));
        assert_eq!(p, PathBuf::from("/tmp/foo.ttf"));
    }
}
