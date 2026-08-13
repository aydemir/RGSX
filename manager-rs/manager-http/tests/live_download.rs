//! TASK-002l — librqbit uçtan uca canlı indirme (HTTP katmanı).
//!
//! Gerçek bir torrent (Sintel) `manager-bin` HTTP sunucusu üzerinden librqbit ile
//! indirilir ve `dest_path`'e sonlanır. Ağ/peer bağımlı olduğundan varsayılan
//! olarak `#[ignore]`; yalnızca açıkça çalıştırılır:
//!
//! ```sh
//! RGSX_LIVE_TORRENT_TEST=1 \
//!   CARGO_TARGET_DIR=/tmp/rgsx-target \
//!   cargo test -p manager-http --test live_download -- --ignored
//! ```
//!
//! Bu test, `examples/live_torrent.rs` (engine-seviyesi) kanıtını HTTP katmanı
//! (router → `download` handler → `TorrentBackend::download_torrent`) üzerinden
//! tekrarlar; yani librqbit'in gerçekten **manager akışında** indirdiğini gösterir.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use manager_bridge::TorrentBackend;
use manager_http::{router, AppState};
use manager_torrent::LibrqbitEngine;
use serde_json::json;
use tower::ServiceExt;

/// Sintel — public domain, küçük, iyi seed'li referans torrent.
const SINTEL_MAGNET: &str =
    "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel";

fn live_app(output: &std::path::Path, downloads: &std::path::Path) -> Router {
    let engine = LibrqbitEngine::new(
        output.to_path_buf(),
        downloads.to_string_lossy().to_string(),
        downloads.to_string_lossy().to_string(),
    );
    let mut state = AppState::empty();
    state.bridge = Some(Arc::new(engine) as Arc<dyn TorrentBackend>);
    router(state)
}

#[tokio::test]
#[ignore]
async fn live_download_sintel_via_http_librqbit() {
    if std::env::var("RGSX_LIVE_TORRENT_TEST").is_err() {
        eprintln!("RGSX_LIVE_TORRENT_TEST set değil → atlanıyor (ignored)");
        return;
    }

    let tmp = std::env::temp_dir().join(format!("rgsx_live_{}", std::process::id()));
    let output = tmp.join("out");
    let downloads = tmp.join("dl");
    std::fs::create_dir_all(&output).unwrap();
    std::fs::create_dir_all(&downloads).unwrap();
    let dest = downloads.join("Sintel.mp4");

    let app = live_app(&output, &downloads);
    let body = json!({
        "url": SINTEL_MAGNET,
        "game_name": "Sintel",
        "platform": "PC",
        "dest_path": dest.to_string_lossy().to_string(),
    });
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/download")
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // Arka plan indirme tamamlanana kadar `dest_path`'i bekle.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(900);
    loop {
        if dest.exists() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "Sintel 900s içinde indirilemedi (ağ/peer bağımlı)"
        );
        tokio::time::sleep(Duration::from_millis(1000)).await;
    }
    let meta = std::fs::metadata(&dest).unwrap();
    assert!(meta.len() > 0, "indirilen dosya boş");

    let _ = std::fs::remove_dir_all(&tmp);
}
