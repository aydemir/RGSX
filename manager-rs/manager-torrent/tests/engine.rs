//! manager-torrent unit testleri — librqbit WebUI'siz olduğundan `call` dispatch'i
//! session spawn etmeden (offline) doğrulanır; ensure_running sonrası için
//! canlı test ayrıdır (examples/live_torrent.rs).
#![forbid(unsafe_code)]

use std::path::PathBuf;

use manager_bridge::TorrentBackend;
use manager_torrent::LibrqbitEngine;
use serde_json::json;

async fn engine() -> LibrqbitEngine {
    LibrqbitEngine::new(
        PathBuf::from("/tmp/unit_dl"),
        "/tmp/unit_downloads".to_string(),
        "/tmp/unit_logs".to_string(),
    )
}

#[tokio::test]
async fn ping_returns_pong() {
    let e = engine().await;
    assert_eq!(e.ping().await.unwrap(), "pong");
}

#[tokio::test]
async fn status_stopped_before_running() {
    let e = engine().await;
    let s = e.status().await.unwrap();
    assert_eq!(s.state, "STOPPED");
    assert!(s.available);
}

#[tokio::test]
async fn only_listed_methods_dispatch() {
    let e = engine().await;
    // Sözleşmede olmayan metod JSON-RPC "Method not found" koduna (-32601) düşer.
    let err = e.call("nope", json!({})).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Method not found: nope"), "{msg}");
}

#[tokio::test]
async fn password_status_matches_python_contract() {
    let e = engine().await;
    let v = e.get_password_status().await.unwrap();
    assert_eq!(v["available"], true);
    assert_eq!(v["using_default"], false);
    assert_eq!(v["secured"], true);
    assert_eq!(v["mode"], "embedded");
}

#[tokio::test]
async fn change_password_reports_embedded_mode() {
    let e = engine().await;
    // librqbit'te şifre kavramı yok — (false, "embedded_mode") sözleşmesi.
    let (ok, msg) = e.change_webui_password("RGSXqbt678").await.unwrap();
    assert!(!ok);
    assert_eq!(msg, "embedded_mode");
}

#[tokio::test]
async fn app_paths_returned_from_ctor() {
    let e = engine().await;
    let (dl, logs) = e.get_app_paths().await.unwrap();
    assert_eq!(dl, "/tmp/unit_downloads");
    assert_eq!(logs, "/tmp/unit_logs");
}

#[tokio::test]
async fn engine_name_is_librqbit() {
    let e = engine().await;
    assert_eq!(e.engine(), "librqbit");
}