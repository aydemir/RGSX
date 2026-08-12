//! Contract tipleri: /api/* ve /api/events (SSE) sözleşme yapıları.
//!
//! TASK-002b — `tests/test_api_contract.py` ile 1:1 kilitlenen yanıt şablonları.
//! Kaynak: `ports/RGSX/rgsx_web/handlers.py` (`_send_json`/`_set_headers`) ve
//! `ports/RGSX/rgsx_manager.py` (`_sse_event`, `_build_snapshot`).
//!
//! Not: serde_json Map sırasız (BTree) — kapalı anahtarlar alfabetik gider;
//! Python testleri dict eşitliği üzerinden karşılaştırdığı için bu fark etmez.

use serde_json::{json, Value};

/// Başarılı zarfa alan ekler: `{"success": true, ...alanlar}`.
///
/// Python `_send_json({"success": True, **extra})` davranışını çoğaltır.
pub fn ok(mut extra: Value) -> Value {
    let mut base = json!({ "success": true });
    if let (Value::Object(a), Value::Object(b)) = (&mut base, &mut extra) {
        a.append(b);
    }
    base
}

/// Hata zarfi: `{"success": false, "error": <msg>}` (Python hata şablonu).
pub fn error(msg: impl Into<String>) -> Value {
    json!({ "success": false, "error": msg.into() })
}

/// SSE olay formatı (`rgsx_manager.py:81`):
///
/// ```text
/// event: <type>
/// data: <json>
///
/// ```
///
/// `json.dumps(data, ensure_ascii=False, default=str)` ile 1:1; serde_json
/// UTF-8'i kaçırmaz, Python default=str + ASCII kaçışı bu portta gerekmez.
pub fn sse_event(event_type: &str, data: &Value) -> String {
    format!("event: {event_type}\ndata: {}\n\n", data)
}

/// Snapshot yükü (`_build_snapshot`, rgsx_manager.py:86-109).
///
/// Anahtarlar Python dict sırasıyla korunur (okuyucu dahil görünürlük).
pub fn snapshot(history: &Value, queue: &Value, active: bool, progress: &Value, downloaded: &Value) -> Value {
    json!({
        "history": history,
        "queue": queue,
        "active": active,
        "progress": progress,
        "downloaded": downloaded,
    })
}

/// History hata mesajı sadeleştirme (`history.py:17` `_strip_history_error_noise`).
///
/// "Download error X:" öneki + archive.org dosya listesi + kuyruk noktalaması
/// atılır; tam metin history.json'da korunur (detay görünümü).
///
/// ```text
/// "Download error Foo.zip: Accès refusé (HTTP 500). Fichiers disponibles exemples: [...]"
///   -> "Accès refusé (HTTP 500)"
/// ```
pub fn strip_history_error_noise(text: &str) -> String {
    if text.is_empty() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for marker in [
        "Download error ",
        "İndirme hatası ",
        "Erreur téléchargement ",
        "Erreur téléchargement :",
        "Erreur de téléchargement ",
        "Download failed for ",
    ] {
        if let Some(idx) = out.find(marker) {
            let after = &out[idx + marker.len()..];
            out = match after.find(':') {
                Some(sep) => after[sep + 1..].trim().to_string(),
                None => after.trim().to_string(),
            };
            break;
        }
    }
    for list_marker in [
        "Fichiers disponibles exemples:",
        "Available files examples:",
        "Available files example:",
        "Fichiers disponibles:",
        "Available files:",
        "Mevcut dosyalar:",
    ] {
        if let Some(lidx) = out.find(list_marker) {
            out = out[..lidx].trim_end_matches([' ', '.', ':']).to_string();
            break;
        }
    }
    out = out.trim().to_string();
    while matches!(out.chars().last(), Some('.') | Some(':')) {
        out = out[..out.len() - 1].trim_end_matches(['.', ':']).to_string();
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_merges_extra_fields() {
        let v = ok(json!({ "count": 0, "platforms": [] }));
        let obj = v.as_object().unwrap();
        assert_eq!(obj["success"], json!(true));
        assert_eq!(obj["count"], json!(0));
        assert_eq!(obj["platforms"], json!([]));
    }

    #[test]
    fn error_shape_matches_python() {
        assert_eq!(error("Route non trouvée"), json!({ "success": false, "error": "Route non trouvée" }));
    }

    #[test]
    fn sse_event_format_matches_python() {
        let event = sse_event("snapshot", &json!({"active": false}));
        assert!(event.starts_with("event: snapshot\n"), "got: {event}");
        assert!(event.contains("data: "));
        let data_part = event.split("data: ").nth(1).unwrap().trim();
        assert_eq!(serde_json::from_str::<Value>(data_part).unwrap(), json!({"active": false}));
        assert!(event.ends_with("\n\n"));
    }

    #[test]
    fn sse_event_keeps_all_keys() {
        let snap = snapshot(&json!([]), &json!([]), false, &json!({}), &json!({}));
        for key in ["history", "queue", "active", "progress", "downloaded"] {
            assert!(snap.as_object().unwrap().contains_key(key), "missing key {key}");
        }
        let text = sse_event("snapshot", &snap);
        assert!(text.starts_with("event: snapshot\n"));
    }

    #[test]
    fn strip_noise_matches_python_docstring() {
        let noisy = "Download error Crazy Cars ++.zip: Accès refusé (HTTP 500). Fichiers disponibles exemples: ['Addams Family.zip', 'Amiga 500 Tutorial.mp4']";
        assert_eq!(strip_history_error_noise(noisy), "Accès refusé (HTTP 500)");
    }

    #[test]
    fn strip_noise_empty_and_plain() {
        assert_eq!(strip_history_error_noise(""), "");
        assert_eq!(strip_history_error_noise("Download_OK"), "Download_OK");
    }
}