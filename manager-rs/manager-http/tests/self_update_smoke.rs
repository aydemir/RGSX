//! TASK-012m canlı duman testi (Grok önerisi 1).
//! Tam manager-bin'i uçurmadan, yerel bir HTTP sunucu (sahte manifest + binary)
//! kurup gerçek akışı doğrular: check_update → SSE manager_update → indir + SHA256.

use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use manager_http::self_update::{check_update, download_and_verify};
use manager_http::state::StateData;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;

#[derive(Clone)]
struct AppState {
    manifest: Arc<Mutex<Value>>,
    bin: Arc<Vec<u8>>,
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

async fn spawn_mock_server() -> u16 {
    let blob: Vec<u8> = b"fake-manager-binary-for-smoke-test".to_vec();
    let sha = {
        let mut h = Sha256::new();
        h.update(&blob);
        hex(&h.finalize())
    };
    let manifest = Arc::new(Mutex::new(json!({})));
    let bin = Arc::new(blob);
    let state = AppState {
        manifest: manifest.clone(),
        bin: bin.clone(),
    };

    async fn manifest_handler(State(s): State<AppState>) -> (StatusCode, Json<Value>) {
        (StatusCode::OK, Json(s.manifest.lock().unwrap().clone()))
    }
    async fn bin_handler(State(s): State<AppState>) -> Vec<u8> {
        s.bin.to_vec()
    }

    let app = Router::new()
        .route("/manifest", get(manifest_handler))
        .route("/bin", get(bin_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    *manifest.lock().unwrap() = json!({
        "version": "99.0.0",
        "url": format!("http://127.0.0.1:{port}/bin"),
        "sha256": sha
    });

    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    port
}

#[tokio::test]
async fn smoke_full_update_flow_publishes_sse_and_downloads_with_sha() {
    let port = spawn_mock_server().await;
    let manifest_url = format!("http://127.0.0.1:{port}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);

    let (sender, mut rx) = broadcast::channel::<String>(16);
    let state_data = Arc::new(RwLock::new(StateData::empty()));

    tokio::spawn(check_update(sender.clone(), state_data.clone()));

    // SSE yayıını (manager_update) al
    let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("timeout: manager_update SSE gelmedi")
        .expect("channel kapandı");
    assert!(msg.starts_with("event: manager_update\n"), "got: {msg}");
    let data_str = msg.split("data: ").nth(1).unwrap().trim();
    let data: Value = serde_json::from_str(data_str).unwrap();
    assert_eq!(data["available"], true);
    assert_eq!(data["version"], "99.0.0");

    // StateData snapshot alanı da doldu
    let sd = state_data.write().unwrap();
    let mu = sd.manager_update.as_ref().expect("manager_update kaydedilmedi");
    assert_eq!(mu["version"], "99.0.0");
    drop(sd);

    // İndirme + SHA256 doğrulama (Faz 4)
    let bin_url = data["url"].as_str().unwrap().to_string();
    let sha = data["sha256"].as_str().unwrap().to_string();
    let path = download_and_verify(&bin_url, Some(sha.as_str()))
        .await
        .expect("download+verify başarısız");
    assert!(std::path::Path::new(&path).exists(), "indirilen dosya yok");

    // SHA uyuşmazlığı reddedilmeli
    let err = download_and_verify(&bin_url, Some("deadbeefdeadbeef")).await;
    assert!(err.is_err(), "yanlış SHA kabul edildi!");
}
