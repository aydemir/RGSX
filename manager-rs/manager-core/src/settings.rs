//! Native ayar şeması — `ports/RGSX/rgsx_settings.py` + `config.py` portu (Faz 12f).
//!
//! TASK-002s: `rgsx_settings.json` için typed `Settings` struct + `Default`
//! (Python `default_settings` birleşimi) + `load()`/`save()` + `validate()`.
//!
//! Davranış kuralları (Python ile birebir):
//! - `language` key'i dosyada yoksa **enjekte edilmez** ("key yok = kullanıcı seçimi yok").
//!   Bu yüzden `Option<String>` + `skip_serializing_if = "none"`.
//! - `auto_extract` / `api_keys` / `web_service_at_boot` / `custom_dns_at_boot` ayrı
//!   mekanizmalardır (ayrı dosya / systemd); native save'de dosyaya yazılmaz.
//! - `game_filters` ve bilinmeyen ek alanlar `extra` ile korunur (round-trip).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Native persist aç/kapa (`RGSX_NATIVE_SETTINGS=1`). Kapalı → handler'lar Python
/// proxy / placeholder davranışına düşer (kesintisiz göç, risk sıfır).
pub fn native_enabled() -> bool {
    std::env::var("RGSX_NATIVE_SETTINGS").map(|v| v == "1").unwrap_or(false)
}

/// `rgsx_settings.json` yolu: `RGSX_SETTINGS_PATH` > `RGSX_DATA_DIR/rgsx_settings.json`.
/// Python `config.RGSX_SETTINGS_PATH = SAVE_FOLDER/rgsx_settings.json` eşleniği.
pub fn settings_path() -> PathBuf {
    if let Ok(p) = std::env::var("RGSX_SETTINGS_PATH") {
        return PathBuf::from(p);
    }
    let data_dir = std::env::var("RGSX_DATA_DIR").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(data_dir).join("rgsx_settings.json")
}

// ---------------------------------------------------------------------------
// Alt şemalar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Accessibility {
    #[serde(default = "default_font_scale")]
    pub font_scale: f64,
    #[serde(default = "default_footer_font_scale")]
    pub footer_font_scale: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Display {
    #[serde(default = "default_grid")]
    pub grid: String,
    #[serde(default = "default_font_family")]
    pub font_family: String,
    #[serde(default)]
    pub monitor: i32,
    #[serde(default = "default_true")]
    pub fullscreen: bool,
    #[serde(default)]
    pub light_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Symlink {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub target_directory: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sources {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub custom_url: String,
}

// ---------------------------------------------------------------------------
// Ana şema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// `Option` + `skip_serializing_if` → dosyada yoksa enjekte edilmez.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default)]
    pub music_enabled: bool,
    #[serde(default)]
    pub accessibility: Accessibility,
    #[serde(default)]
    pub display: Display,
    #[serde(default)]
    pub symlink: Symlink,
    #[serde(default)]
    pub sources: Sources,
    #[serde(default)]
    pub show_unsupported_platforms: bool,
    #[serde(default)]
    pub allow_unknown_extensions: bool,
    #[serde(default)]
    pub nintendo_layout: bool,
    #[serde(default)]
    pub roms_folder: String,
    #[serde(default)]
    pub web_service_at_boot: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_gamelist_update: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_gamelist_prompt_remote_update: Option<String>,
    #[serde(default = "default_sort")]
    pub global_sort_option: String,
    #[serde(default)]
    pub platform_custom_paths: HashMap<String, String>,
    #[serde(default = "default_max_dl")]
    pub max_simultaneous_downloads: u32,
    /// `game_filters` ve bilinmeyen ek alanlar (round-trip koruması).
    #[serde(flatten, default)]
    pub extra: HashMap<String, serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Varsayılan değerler (Python `default_settings` birleşimi)
// ---------------------------------------------------------------------------

fn default_font_scale() -> f64 {
    1.0
}
fn default_footer_font_scale() -> f64 {
    1.5
}
fn default_grid() -> String {
    "3x4".to_string()
}
fn default_font_family() -> String {
    "pixel".to_string()
}
fn default_true() -> bool {
    true
}
fn default_mode() -> String {
    "rgsx".to_string()
}
fn default_sort() -> String {
    "name_asc".to_string()
}
fn default_max_dl() -> u32 {
    5
}

impl Default for Accessibility {
    fn default() -> Self {
        Accessibility {
            font_scale: default_font_scale(),
            footer_font_scale: default_footer_font_scale(),
        }
    }
}

impl Default for Display {
    fn default() -> Self {
        Display {
            grid: default_grid(),
            font_family: default_font_family(),
            monitor: 0,
            fullscreen: default_true(),
            light_mode: false,
        }
    }
}

impl Default for Sources {
    fn default() -> Self {
        Sources {
            mode: default_mode(),
            custom_url: String::new(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            language: Some("en".to_string()),
            music_enabled: true,
            accessibility: Accessibility::default(),
            display: Display::default(),
            symlink: Symlink::default(),
            sources: Sources::default(),
            show_unsupported_platforms: false,
            allow_unknown_extensions: false,
            nintendo_layout: false,
            roms_folder: String::new(),
            web_service_at_boot: false,
            last_gamelist_update: None,
            last_gamelist_prompt_remote_update: None,
            global_sort_option: default_sort(),
            platform_custom_paths: HashMap::new(),
            max_simultaneous_downloads: default_max_dl(),
            extra: HashMap::new(),
        }
    }
}

impl Settings {
    /// Dosyadan yükle; yoksa/corrupt ise Python `default_settings` birleşimini döndür.
    /// Yüklenen geçici alanlar (`auto_extract` vb.) temizlenir.
    pub fn load() -> Settings {
        let path = settings_path();
        match std::fs::read_to_string(&path) {
            Ok(txt) => match serde_json::from_str::<Settings>(&txt) {
                Ok(mut s) => {
                    s.normalize_transient();
                    s
                }
                Err(_) => Settings::default(),
            },
            Err(_) => Settings::default(),
        }
    }

    /// `rgsx_settings.json`'a yazar (geçici alanlar hariç). Üst dizin oluşturulur.
    pub fn save(&self) -> std::io::Result<()> {
        let path = settings_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut v = serde_json::to_value(self)?;
        if let Some(obj) = v.as_object_mut() {
            // Python `_api_settings_post` bu alanları save öncesi `del` eder.
            obj.remove("auto_extract");
            obj.remove("api_keys");
            obj.remove("web_service_at_boot");
            obj.remove("custom_dns_at_boot");
        }
        std::fs::write(&path, serde_json::to_string_pretty(&v)?)
    }

    /// Geçici/ayrı-mekanizma alanlarını `extra`'dan düşür (dosyadan okunmuşsa).
    fn normalize_transient(&mut self) {
        self.extra.remove("auto_extract");
        self.extra.remove("api_keys");
        self.extra.remove("web_service_at_boot");
        self.extra.remove("custom_dns_at_boot");
    }

    /// Tip/invariant doğrulaması. Python'ın yapmadığı ama native'nin garanti ettiği
    /// kontroller — mevcut veriyi **reddetmeyecek** kadar minimal tutuldu.
    pub fn validate(&self) -> Result<(), String> {
        if self.accessibility.font_scale <= 0.0 {
            return Err("accessibility.font_scale doit être > 0".to_string());
        }
        if self.accessibility.footer_font_scale <= 0.0 {
            return Err("accessibility.footer_font_scale doit être > 0".to_string());
        }
        if self.max_simultaneous_downloads < 1 {
            return Err("max_simultaneous_downloads doit être >= 1".to_string());
        }
        if self.display.monitor < 0 {
            return Err("display.monitor doit être >= 0".to_string());
        }
        Ok(())
    }
}

/// `GET /api/settings` `system_info` bloğu (env tabanlı; saf-Rust modda platform
/// sayısı yerel `systems_list.json`'dan üretilir).
pub fn system_info() -> serde_json::Value {
    let system = std::env::consts::OS;
    let roms_folder = std::env::var("RGSX_ROMS_FOLDER").unwrap_or_default();
    serde_json::json!({
        "system": system,
        "roms_folder": roms_folder,
        "platforms_count": count_native_platforms()
    })
}

/// Saf-Rust modda (katalog proxy yok) `platforms_count`'u yerel `systems_list.json`
/// dizisinden üretir; dosya/veri yoksa 0.
fn count_native_platforms() -> u32 {
    let data_dir = match std::env::var("RGSX_DATA_DIR") {
        Ok(d) if !d.is_empty() => d,
        _ => return 0,
    };
    let path = std::path::Path::new(&data_dir).join("systems_list.json");
    match std::fs::read_to_string(&path) {
        Ok(txt) => serde_json::from_str::<serde_json::Value>(&txt)
            .ok()
            .and_then(|v| v.as_array().map(|a| a.len() as u32))
            .unwrap_or(0),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_python_default_settings() {
        let s = Settings::default();
        assert_eq!(s.language.as_deref(), Some("en"));
        assert!(s.music_enabled);
        assert_eq!(s.accessibility.font_scale, 1.0);
        assert_eq!(s.accessibility.footer_font_scale, 1.5);
        assert_eq!(s.display.grid, "3x4");
        assert_eq!(s.display.font_family, "pixel");
        assert!(s.display.fullscreen);
        assert!(!s.display.light_mode);
        assert!(!s.symlink.enabled);
        assert_eq!(s.sources.mode, "rgsx");
        assert!(!s.show_unsupported_platforms);
        assert!(!s.allow_unknown_extensions);
        assert!(!s.nintendo_layout);
        assert_eq!(s.global_sort_option, "name_asc");
        assert_eq!(s.max_simultaneous_downloads, 5);
        assert!(s.platform_custom_paths.is_empty());
    }

    #[test]
    fn language_absent_not_injected() {
        // Dosyada language YOK → yükleme sonrası None kalır (Python "key yok" kuralı).
        let s: Settings = serde_json::from_str(r#"{"music_enabled": false}"#).unwrap();
        assert_eq!(s.language, None);
        let v = serde_json::to_value(&s).unwrap();
        assert!(v.get("language").is_none());
    }

    #[test]
    fn extra_fields_preserved_roundtrip() {
        let s: Settings =
            serde_json::from_str(r#"{"game_filters": {"hide_downloaded": true}}"#).unwrap();
        assert_eq!(s.extra.get("game_filters"), Some(&serde_json::json!({"hide_downloaded": true})));
    }

    #[test]
    fn transient_keys_not_persisted() {
        let dir = std::env::temp_dir().join("rgsx_settings_test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("rgsx_settings.json");
        std::env::set_var("RGSX_SETTINGS_PATH", &p);
        let mut s = Settings::default();
        s.extra
            .insert("api_keys".into(), serde_json::json!({"realdebrid": "x"}));
        s.save().unwrap();
        let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert!(v.get("api_keys").is_none());
        // Bilinen alanlar korunur.
        assert_eq!(v.get("language").and_then(|x| x.as_str()), Some("en"));
    }

    #[test]
    fn validate_rejects_bad_invariants() {
        let mut s = Settings::default();
        s.accessibility.font_scale = 0.0;
        assert!(s.validate().is_err());
        let mut s = Settings::default();
        s.max_simultaneous_downloads = 0;
        assert!(s.validate().is_err());
    }

    #[test]
    fn load_missing_file_returns_defaults() {
        std::env::set_var("RGSX_SETTINGS_PATH", "/nonexistent/rgsx_settings.json");
        let s = Settings::load();
        assert_eq!(s.language.as_deref(), Some("en"));
        assert_eq!(s.max_simultaneous_downloads, 5);
    }
}
