//! TASK-012m Faz 5 - canli duman testi (Grok onerisi: once guvenli zincir, apply kapali).
//!
//! Gercek manager_http::router uzerinden HTTP/SSE akisini dogrular:
//! 1. Manifest -> manager_update SSE + snapshot (banner gorunur).
//! 2. POST /api/manager-update/download -> kuyruga girer (non-blocking), streaming indirme.
//! 3. manager_update stage downloading -> ready; temp dosya + SHA256 dogrulanir.
//! 4. Iptal: indirme surerken POST /api/queue/remove -> gorev temizlenir, stage available doner.
//! 5. Apply kapisi: RGSX_SELF_APPLY YOK -> POST /api/manager-update/apply reddedilir.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use manager_http::self_update::check_update;
use manager_http::{router, AppState};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Clone)]
struct MockState {
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
    let blob: Vec<u8> = "fake-manager-binary-for-faz5-smoke-test-0123456789".as_bytes().to_vec();
    let sha = {
        let mut h = Sha256::new();
        h.update(&blob);
        hex(&h.finalize())
    };
    let bin = Arc::new(blob);
    let manifest = Arc::new(Mutex::new(json!({})));
    let st = MockState { manifest: manifest.clone(), bin: bin.clone() };

    async fn manifest_handler(State(s): State<MockState>) -> (StatusCode, Json<Value>) {
        (StatusCode::OK, Json(s.manifest.lock().unwrap().clone()))
    }
    async fn bin_handler(State(s): State<MockState>) -> (StatusCode, Body) {
        // Govde gonderilmeden once bekle: boylece iptal testi "downloading"
        // asamasini gorup /api/queue/remove yapabilsin (tek chunk, decode sorunsuz).
        tokio::time::sleep(Duration::from_millis(300)).await;
        (StatusCode::OK, Body::from((*s.bin).clone()))
    }

    let app = Router::new()
        .route("/manifest", get(manifest_handler))
        .route("/bin", get(bin_handler))
        .with_state(st);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    *manifest.lock().unwrap() = json!({
        "version": "99.0.0",
        "url": format!("http://127.0.0.1:{port}/bin"),
        "sha256": sha
    });
    tokio::spawn(async move { let _ = axum::serve(listener, app).await; });
    port
}
async fn spawn_manager_http(state: AppState) -> u16 {
    let app = router(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move { let _ = axum::serve(listener, app).await; });
    port
}

async fn get_manager_update(port: u16) -> Value {
    let c = reqwest::Client::new();
    let r = c.get(format!("http://127.0.0.1:{port}/api/manager-update")).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    v["update"].clone()
}

async fn wait_stage(port: u16, want: &str, timeout: Duration) -> Value {
    let start = std::time::Instant::now();
    loop {
        let mu = get_manager_update(port).await;
        if mu.get("stage").and_then(|s| s.as_str()) == Some(want) {
            return mu;
        }
        if mu.get("stage").and_then(|s| s.as_str()) == Some("failed") {
            panic!("manager_update failed: {mu}");
        }
        if start.elapsed() > timeout {
            panic!("timeout beklenen stage={want}, son durum: {mu}");
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn faz5_smoke_download_chain_green_apply_closed() {
    let mock = spawn_mock_server().await;
    let manifest_url = format!("http://127.0.0.1:{mock}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);
    std::env::remove_var("RGSX_SELF_APL");
    std::env::remove_var("RGSX_SERVICE");

    let state = AppState::empty();
    let mport = spawn_manager_http(state.clone()).await;

    let events = state.events.clone();
    let sd = state.data.clone();
    tokio::spawn(check_update(events, sd));
    let avail = wait_stage(mport, "available", Duration::from_secs(5)).await;
    assert_eq!(avail["available"].as_bool(), Some(true));
    assert_eq!(avail["version"].as_str(), Some("99.0.0"));
    println!("[faz5-smoke] 1) manifest -> available v{} (banner)", avail["version"]);

    let c = reqwest::Client::new();
    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/download")).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert!(v["success"].as_bool().unwrap_or(false));
    assert!(v["ok"].as_bool().unwrap_or(false));
    assert!(v["queued"].as_bool().unwrap_or(false));
    println!("[faz5-smoke] 2) download queued (non-blocking)");

    let _downloading = wait_stage(mport, "downloading", Duration::from_secs(5)).await;
    let ready = wait_stage(mport, "ready", Duration::from_secs(10)).await;
    let path = ready["path"].as_str().expect("ready ama path yok");
    assert!(std::path::Path::new(path).exists(), "indirilen temp yok: {path}");
    println!("[faz5-smoke] 3) downloading->ready, temp={path}");

    let bin_bytes = std::fs::read(path).unwrap();
    let mut h = Sha256::new();
    h.update(&bin_bytes);
    let actual = hex(&h.finalize());
    let expected = avail["sha256"].as_str().unwrap();
    assert_eq!(actual.to_lowercase(), expected.to_lowercase(), "temp SHA uyumsuz");
    println!("[faz5-smoke] 3b) SHA256 dogrulandi: {actual}");

    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/apply")).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert!(!v["ok"].as_bool().unwrap_or(true), "apply kapaliyken OK dondu!");
    let err = v["error"].as_str().unwrap_or("");
    assert!(err.contains("devre d") && err.contains("RGSX_SELF_APPLY"), "beklenen devre disi hatasi, got: {err}");
    println!("[faz5-smoke] 4) apply kapali -> reddedildi: {err}");
}

#[tokio::test]
async fn faz5_smoke_cancel_while_downloading() {
    let mock = spawn_mock_server().await;
    let manifest_url = format!("http://127.0.0.1:{mock}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);
    std::env::remove_var("RGSX_SELF_APL");
    std::env::remove_var("RGSX_SERVICE");

    let state = AppState::empty();
    let mport = spawn_manager_http(state.clone()).await;

    let events = state.events.clone();
    let sd = state.data.clone();
    tokio::spawn(check_update(events, sd));
    let avail = wait_stage(mport, "available", Duration::from_secs(5)).await;
    assert_eq!(avail["available"].as_bool(), Some(true));

    let c = reqwest::Client::new();
    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/download")).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert!(v["ok"].as_bool().unwrap_or(false));

    let _dl = wait_stage(mport, "downloading", Duration::from_secs(5)).await;
    let r = c.post(format!("http://127.0.0.1:{mport}/api/queue/remove")).json(&json!({ "task_id": "manager-update" })).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert!(v["success"].as_bool().unwrap_or(false));

    let reverted = wait_stage(mport, "available", Duration::from_secs(5)).await;
    assert_eq!(reverted["stage"].as_str(), Some("available"));
    assert!(reverted.get("path").is_none() || reverted["path"].is_null(), "iptal sonrasi path kaldi: {:?}", reverted.get("path"));
    println!("[faz5-smoke] cancel) indirme iptal edildi, stage available dondu");
}
