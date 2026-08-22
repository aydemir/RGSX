//! TASK-012h Faz 2c - TVUI tarafı SSE istemcisi (senkron, ureq).
//!
//! manager-http `/api/events` akisini dinler; `catalog_update` olaylarini
//! paylasilan `TvuiState`'e yazar. SDL2 dongusu bunu okuyup loading bar'ini cizer.
//! Senkron olmasi bilincli: SDL2 event loop tek thread, async/tokio agirligi gereksiz.

use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// TVUI acilis durumu (loading bar kaynagi). SDL2 dongusu ile SSE thread'i
/// arasinda `Arc<Mutex<>>` ile paylasilir.
#[derive(Debug, Clone, Default)]
pub struct TvuiState {
    pub loading: bool,
    pub pct: i64,
    pub stage: String,
    pub ready: bool,
    pub error: Option<String>,
}

pub type SharedTvuiState = Arc<Mutex<TvuiState>>;

/// Tek bir SSE cercevesini ayristirir: `event: <type>\ndata: <json>\n\n` bloklarindan
/// `(event_type, json)` dondurur. `event:`/`data:` satirlari olmadan `None`.
pub fn parse_sse_frame(buf: &str) -> Option<(String, serde_json::Value)> {
    let mut event = String::new();
    let mut data = String::new();
    for line in buf.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data = rest.trim().to_string();
        }
    }
    if event.is_empty() || data.is_empty() {
        return None;
    }
    serde_json::from_str(&data).ok().map(|v| (event, v))
}

fn apply_catalog_update(state: &SharedTvuiState, data: &serde_json::Value) {
    let mut s = state.lock().unwrap();
    s.loading = true;
    if let Some(stage) = data.get("stage").and_then(|v| v.as_str()) {
        s.stage = stage.to_string();
        if stage == "ready" {
            let ok = data.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            s.ready = true;
            s.loading = false;
            s.pct = if ok { 100 } else { s.pct };
            if !ok {
                let reason = data
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("bilinmiyor");
                s.error = Some(format!("katalog hazirlanamadi: {reason}"));
            }
        }
    }
    if let Some(pct) = data.get("pct").and_then(|v| v.as_i64()) {
        s.pct = pct;
    }
}

/// `port` izerindeki manager-http'e SSE baglanir, `catalog_update` olaylarini `state`'e yazar.
/// Baglanti kurulamazsa `state.error` set edip doner (UI yine render eder, bar 0'da kalir).
pub fn start_catalog_watcher(port: u16, state: SharedTvuiState) {
    let url = format!("http://127.0.0.1:{port}/api/events");
    // Yalnizca connect timeout; SSE akisi uzun omurlu oldugu icin read timeout YOK.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .build();
    let resp = match agent.get(&url).call() {
        Ok(r) => r,
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.error = Some(format!("SSE baglanti hatasi: {e}"));
            return;
        }
    };
    let reader = std::io::BufReader::new(resp.into_reader());
    let mut acc = String::new();
    for line in reader.lines().map(|l| l.unwrap_or_default()) {
        if line.is_empty() {
            if let Some((ev, data)) = parse_sse_frame(&acc) {
                if ev == "catalog_update" {
                    apply_catalog_update(&state, &data);
                }
            }
            acc.clear();
        } else {
            acc.push_str(&line);
            acc.push('\n');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_frame_extracts_event_and_data() {
        let frame = "event: catalog_update\ndata: {\"stage\":\"download\",\"pct\":42,\"total\":1000}\n\n";
        let (ev, data) = parse_sse_frame(frame).expect("frame cozulmeli");
        assert_eq!(ev, "catalog_update");
        assert_eq!(data["pct"], 42);
        assert_eq!(data["stage"], "download");
    }

    #[test]
    fn parse_sse_frame_returns_none_without_event_or_data() {
        assert!(parse_sse_frame("data: {\"a\":1}\n\n").is_none());
        assert!(parse_sse_frame("event: x\n\n").is_none());
        assert!(parse_sse_frame("").is_none());
    }

    #[test]
    fn apply_catalog_update_sets_pct_and_ready() {
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_catalog_update(
            &state,
            &serde_json::json!({"stage":"download","pct":10,"total":500}),
        );
        assert_eq!(state.lock().unwrap().pct, 10);
        apply_catalog_update(
            &state,
            &serde_json::json!({"stage":"ready","success":true}),
        );
        let s = state.lock().unwrap();
        assert!(s.ready);
        assert!(!s.loading);
        assert_eq!(s.pct, 100);
    }
}
