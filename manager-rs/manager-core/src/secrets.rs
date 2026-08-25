//! Secret redaction — Python `utils/security.py` parity (TASK-002-gap-13).
//!
//! `redact_sensitive_settings` / `_redact_settings_file_text` birebir Rust portu:
//! hassas ayar anahtarlarının değerlerini `<redacted>` ile değiştirir. Özyinelemeli,
//! orijinal değerleri mutasyona uğratmaz.

use regex::Regex;
use serde_json::Value;

/// Redakte edilmiş değer yerine konan sabit (Python `_REDACTED_PLACEHOLDER`).
pub const REDACTED_PLACEHOLDER: &str = "<redacted>";

/// Python `_SENSITIVE_SETTING_KEY_RE` ile birebir aynı desen.
fn sensitive_key_re() -> &'static Regex {
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(password|passwd|secret|token|credential|api[_-]?key|(?:^|[_\-])key$)")
            .expect("geçerli regex")
    })
}

/// Anahtar hassas mı? (password/secret/token/api_key/.../_key)
fn is_sensitive_key(key: &str) -> bool {
    sensitive_key_re().is_match(key)
}

/// `value` içindeki hassas alanları özyinelemeli redakte eder, **kopya** döndürür.
/// Orijinal `value` mutasyona uğramaz (Python `redact_sensitive_settings` parity'si).
pub fn redact_secrets(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                if is_sensitive_key(k) {
                    out.insert(k.clone(), Value::String(REDACTED_PLACEHOLDER.to_string()));
                } else {
                    out.insert(k.clone(), redact_secrets(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_secrets).collect()),
        other => other.clone(),
    }
}

/// JSON metnini (örn. `rgsx_settings.json` içeriği) redakte edilmiş metne çevirir.
/// Parse hatasında orijinal metni olduğu gibi döndürür (Python `_redact_settings_file_text`
/// parity'si — dosya bozuksa ham içerik eklenir).
pub fn redact_json_text(text: &str) -> String {
    match serde_json::from_str::<Value>(text) {
        Ok(v) => {
            serde_json::to_string_pretty(&redact_secrets(&v)).unwrap_or_else(|_| text.to_string())
        }
        Err(_) => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_password_field() {
        let data = json!({ "qbittorrent_webui_password": "s3cret", "language": "tr" });
        let out = redact_secrets(&data);
        assert_eq!(out["qbittorrent_webui_password"], json!("<redacted>"));
        assert_eq!(out["language"], json!("tr"));
    }

    #[test]
    fn redacts_nested_sensitive() {
        let data =
            json!({ "sources": { "mode": "rgsx", "custom_url": "https://x", "api_key": "k123" } });
        let out = redact_secrets(&data);
        assert_eq!(out["sources"]["api_key"], json!(REDACTED_PLACEHOLDER));
        assert_eq!(out["sources"]["mode"], json!("rgsx"));
        assert_eq!(out["sources"]["custom_url"], json!("https://x"));
    }

    #[test]
    fn redacts_secret_token_credential() {
        let data = json!({ "webhook_secret": "a", "refresh_token": "b", "proxy_credentials": { "user": "u", "passwd": "p" } });
        let out = redact_secrets(&data);
        assert_eq!(out["webhook_secret"], json!("<redacted>"));
        assert_eq!(out["refresh_token"], json!("<redacted>"));
        assert_eq!(out["proxy_credentials"], json!("<redacted>"));
    }

    #[test]
    fn redacts_items_in_lists() {
        let data = json!({ "servers": [ { "name": "x", "apikey": "abc" }, { "name": "y" } ] });
        let out = redact_secrets(&data);
        assert_eq!(out["servers"][0]["apikey"], json!("<redacted>"));
        assert_eq!(out["servers"][0]["name"], json!("x"));
        assert_eq!(out["servers"][1]["name"], json!("y"));
    }

    #[test]
    fn leaves_non_sensitive_untouched() {
        let data = json!({
            "region_priority": ["USA"],
            "hide_downloaded": false,
            "manager_port": 5000,
            "platform_custom_paths": { "ps2": "/roms/ps2" },
            "keyboard_layout": "tr",
        });
        assert_eq!(redact_secrets(&data), data);
    }

    #[test]
    fn does_not_mutate_original() {
        let data = json!({ "qbittorrent_webui_password": "x", "nested": { "token": "y" } });
        let _ = redact_secrets(&data);
        assert_eq!(data["qbittorrent_webui_password"], json!("x"));
        assert_eq!(data["nested"]["token"], json!("y"));
    }

    #[test]
    fn json_text_redacts_and_is_pretty() {
        let text =
            r#"{"language":"en","qbittorrent_webui_password":"s3cret!","sources":{"mode":"rgsx"}}"#;
        let out = redact_json_text(text);
        assert!(out.contains("<redacted>"));
        assert!(!out.contains("s3cret!"));
    }

    #[test]
    fn json_text_fallback_on_parse_error() {
        let broken = "{ not valid json";
        assert_eq!(redact_json_text(broken), broken);
    }
}
