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
async fn only_listed_methods_dispatch() {
    let e = engine().await;
    // Sözleşmede olmayan metod JSON-RPC "Method not found" koduna (-32601) düşer.
    let err = e.call("nope", json!({})).await.unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("Method not found: nope"), "{msg}");
}

#[tokio::test]
async fn retired_qbittorrent_methods_dispatch_method_not_found() {
    // TASK-013: qBittorrent-kavramlı metodlar trait'ten ve call() dispatch'inden
    // söküldü — hepsi artık -32601 "Method not found"a düşmeli.
    let e = engine().await;
    for m in [
        "ping",
        "status",
        "is_available",
        "ensure_running",
        "get_webui_url",
        "get_password_status",
        "change_webui_password",
        "regenerate_qbittorrent_password",
    ] {
        let err = e.call(m, json!({})).await.unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains(&format!("Method not found: {m}")),
            "{m}: {msg}"
        );
    }
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

#[tokio::test]
async fn resolve_downloaded_file_skips_parts_and_finds_largest_content() {
    let root = std::env::temp_dir().join(format!("rgsx_torrent_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("Sintel")).unwrap();
    std::fs::write(root.join("Sintel/Sintel.de.srt.rqbitpart"), "part").unwrap();
    std::fs::write(root.join("Sintel/Sintel.de.srt"), "small sub").unwrap();
    std::fs::write(root.join("Sintel/Sintel.mp4"), "x".repeat(4096).as_bytes()).unwrap();
    // Sondaki .part artığı: atlanmalı.
    std::fs::write(root.join("Sintel/Sintel.tmp.part"), "partial").unwrap();
    let e = LibrqbitEngine::new(
        root.clone(),
        "/tmp/unit_downloads".to_string(),
        "/tmp/unit_logs".to_string(),
    );
    let found = e.resolve_downloaded_file().await.unwrap();
    assert_eq!(found.file_name().unwrap().to_string_lossy(), "Sintel.mp4");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn link_or_copy_creates_dest_linked_to_src() {
    let root = std::env::temp_dir().join(format!("rgsx_link_test_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let src = root.join("src.iso");
    let dst = root.join("dst.iso");
    std::fs::write(&src, b"data").unwrap();
    manager_torrent::link_or_copy(&src, &dst).unwrap();
    assert!(dst.exists());
    assert_eq!(std::fs::read(&dst).unwrap(), b"data");
    // Hedef zaten varsa üzerine yaz masrafı (önce sil, tekrar bağla).
    std::fs::remove_file(&dst).unwrap();
    std::fs::write(&dst, b"other").unwrap();
    manager_torrent::link_or_copy(&src, &dst).unwrap();
    assert_eq!(std::fs::read(&dst).unwrap(), b"data");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn pause_all_without_active_torrents_returns_zero() {
    let e = engine().await;
    // Session spawn edilir (ensure_running) ama aktif handle yok → 0, hata değil.
    assert_eq!(e.pause_all().await.unwrap(), 0);
}

#[tokio::test]
async fn resume_all_without_active_torrents_returns_zero() {
    let e = engine().await;
    assert_eq!(e.resume_all().await.unwrap(), 0);
}

#[tokio::test]
async fn pause_unknown_task_reports_not_paused() {
    let e = engine().await;
    assert!(!e.is_paused("bilinmeyen-task").await.unwrap());
    // `pause_torrent` kayıtlı handle yoksa sessizce başarılı (Python 1:1: no-op).
    assert!(e.pause_torrent("bilinmeyen-task").await.is_ok());
    assert!(e.resume_torrent("bilinmeyen-task").await.is_ok());
}

#[tokio::test]
async fn pause_resume_dispatched_via_jsonrpc_call() {
    let e = engine().await;
    // `call` sözleşmesi: `pause_all`/`resume_all`/`pause`/`resume` metodları tanımlı
    // olmalı (Python `_BRIDGE_METHODS` simetrisi) — boş map ile `{paused:0}`/`{resumed:0}`.
    let paused = e.call("pause_all", json!({})).await.unwrap();
    assert_eq!(paused["paused"], json!(0));
    let resumed = e.call("resume_all", json!({})).await.unwrap();
    assert_eq!(resumed["resumed"], json!(0));
    let single = e.call("pause", json!({ "task_id": "x" })).await.unwrap();
    assert_eq!(single, json!(null));
    let resumed_one = e.call("resume", json!({ "task_id": "x" })).await.unwrap();
    assert_eq!(resumed_one, json!(null));
    let paused_check = e
        .call("is_paused", json!({ "task_id": "x" }))
        .await
        .unwrap();
    assert_eq!(paused_check, json!(false));
}

#[tokio::test]
async fn cancel_unknown_task_reports_not_found() {
    let e = engine().await;
    // Kayıtlı handle yok → `cancel` false, `cancel_all` 0, sessiz hata değil.
    assert!(!e.cancel_torrent("bilinmeyen-task").await.unwrap());
    assert_eq!(e.cancel_all().await.unwrap(), 0);
}

#[tokio::test]
async fn cancel_dispatched_via_jsonrpc_call() {
    let e = engine().await;
    // `call` sözleşmesi: `cancel`/`cancel_all` metodları tanımlı olmalı — boş map ile
    // `cancel:false`, `cancel_all:{canceled:0}`.
    let single = e.call("cancel", json!({ "task_id": "x" })).await.unwrap();
    assert_eq!(single, json!(false));
    let all = e.call("cancel_all", json!({})).await.unwrap();
    assert_eq!(all["canceled"], json!(0));
}
