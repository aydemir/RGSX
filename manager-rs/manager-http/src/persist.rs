//! TASK-002-gap-10 (A): history.json disk kalıcılığı.
//!
//! Python `history.py` parity'si: startup'ta yükle (geçersiz entry filtrele),
//! değişiklikte atomik yaz (temp + fsync + rename). `history_path = None` ise
//! kalıcılık atlanır (test / env set değil).

use serde_json::Value;
use std::path::Path;

/// `history.json`'ı yükler; parse edilemezse veya dizi değilse boş döner.
/// Geçersiz (obje olmayan) entry'ler elenir (Python `load_history` parity'si).
pub fn load_history(path: &Path) -> Vec<Value> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    let parsed: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    match parsed {
        Value::Array(arr) => arr.into_iter().filter(|e| e.is_object()).collect(),
        _ => Vec::new(),
    }
}

/// `history`'yi atomik olarak `history.json`'a yazar: temp dosya + `sync_all`
/// + `rename` (yerine koyma). Hata olursa yalnızca log'lanır (Python yazım
/// hata cooldown'u parity'si — ana akış etkilenmez).
pub fn save_history(history: &[Value], path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                tracing::warn!("history dizini oluşturulamadı ({}): {e}", parent.display());
            }
        }
    }
    let tmp = path.with_extension("tmp");
    let result = (|| -> std::io::Result<()> {
        let text = serde_json::to_string_pretty(history)?;
        let mut f = std::fs::File::create(&tmp)?;
        use std::io::Write;
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)?;
        Ok(())
    })();
    if let Err(e) = result {
        tracing::warn!("history kaydedilemedi ({}): {e}", path.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn load_filters_non_objects() {
        let dir = std::env::temp_dir().join(format!("rgsx_hist_test_{}.json", std::process::id()));
        std::fs::write(
            &dir,
            r#"[{"game_name":"a","status":"Download_OK"}, "junk", {"no_game_name":true}]"#,
        )
        .unwrap();
        let loaded = load_history(&dir);
        assert_eq!(loaded.len(), 2);
        std::fs::remove_file(&dir).ok();
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = std::env::temp_dir().join(format!("rgsx_hist_rt_{}.json", std::process::id()));
        let hist = vec![json!({"game_name":"x","status":"Download_OK"})];
        save_history(&hist, &dir);
        let loaded = load_history(&dir);
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0]["game_name"], json!("x"));
        std::fs::remove_file(&dir).ok();
    }
}
