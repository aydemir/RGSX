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
use manager_http::self_update::{check_update, recover_update};
use manager_http::{router, AppState};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::Mutex as AsyncMutex;

// 4 test ayni process icinde calisir ve global env (RGSX_SELF_APPLY vb) kullanir;
// yarisi apply'i acar. Cakismamak icin testleri serilestiriyoruz.
static SERIAL: AsyncMutex<()> = AsyncMutex::const_new(());

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
    let _guard = SERIAL.lock().await;
    let mock = spawn_mock_server().await;
    let manifest_url = format!("http://127.0.0.1:{mock}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);
    std::env::remove_var("RGSX_SELF_APPLY");
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
    let _guard = SERIAL.lock().await;
    let mock = spawn_mock_server().await;
    let manifest_url = format!("http://127.0.0.1:{mock}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);
    std::env::remove_var("RGSX_SELF_APPLY");
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

// --- Faz 5 apply (test-copy + hedef override) helpers ---

async fn spawn_mock_server_exe() -> u16 {
    let bytes = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let sha = {
        let mut h = Sha256::new();
        h.update(&bytes);
        hex(&h.finalize())
    };
    let bin = Arc::new(bytes);
    let manifest = Arc::new(Mutex::new(json!({})));
    let st = MockState { manifest: manifest.clone(), bin: bin.clone() };

    async fn manifest_handler(State(s): State<MockState>) -> (StatusCode, Json<Value>) {
        (StatusCode::OK, Json(s.manifest.lock().unwrap().clone()))
    }
    async fn bin_handler(State(s): State<MockState>) -> (StatusCode, Body) {
        tokio::time::sleep(Duration::from_millis(50)).await;
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

fn sha_bytes(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    hex(&h.finalize())
}

#[tokio::test]
async fn faz5_apply_test_copy_replace_and_recover() {
    let _guard = SERIAL.lock().await;
    let exe = std::env::current_exe().unwrap();
    let orig = std::fs::read(&exe).unwrap();
    let mut orig_flipped = orig.clone();
    if let Some(b) = orig_flipped.first_mut() {
        *b ^= 0xFF;
    }
    let dir = std::env::temp_dir().join(format!("rgsx_apply_test_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let target = dir.join("manager-test-copy.exe");
    let old = target.with_extension("old");
    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_file(&old);
    std::fs::write(&target, &orig_flipped).unwrap();

    let mock = spawn_mock_server_exe().await;
    let manifest_url = format!("http://127.0.0.1:{mock}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);
    std::env::set_var("RGSX_SELF_APPLY", "1");
    std::env::set_var("RGSX_SELF_APPLY_TARGET", target.to_str().unwrap());
    std::env::set_var("RGSX_SELF_APPLY_TEST", "1");
    std::env::remove_var("RGSX_SERVICE");

    let state = AppState::empty();
    let mport = spawn_manager_http(state.clone()).await;
    let events = state.events.clone();
    let sd = state.data.clone();
    tokio::spawn(check_update(events, sd));
    wait_stage(mport, "available", Duration::from_secs(5)).await;

    let c = reqwest::Client::new();
    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/download")).send().await.unwrap();
    let _v = r.json::<Value>().await.unwrap();
    wait_stage(mport, "ready", Duration::from_secs(15)).await;

    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/apply")).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert!(v["ok"].as_bool().unwrap_or(false), "apply basarisiz: {v}");

    let target_hash = sha_bytes(&std::fs::read(&target).unwrap());
    let old_hash = sha_bytes(&std::fs::read(&old).unwrap());
    let orig_hash = sha_bytes(&orig);
    let flipped_hash = sha_bytes(&orig_flipped);
    assert_eq!(target_hash.to_lowercase(), orig_hash.to_lowercase(), "hedef indirilenle eslesmiyor");
    assert_eq!(old_hash.to_lowercase(), flipped_hash.to_lowercase(), "eski yedek orijinalle eslesmiyor");
    assert_ne!(target_hash.to_lowercase(), old_hash.to_lowercase(), "degisiklik olmadi (hedef==old)");
    println!("[faz5-apply] replace + .old dogrulandi; relaunch probe tamam");

    std::fs::write(&target, b"CORRUPTED_BINARY_PAYLOAD").unwrap();
    assert_ne!(sha_bytes(&std::fs::read(&target).unwrap()).to_lowercase(), orig_hash.to_lowercase(), "bozuk yazilmadi");
    recover_update(Some(target.clone())).unwrap();
    assert_eq!(sha_bytes(&std::fs::read(&target).unwrap()).to_lowercase(), old_hash.to_lowercase(), "recover .old dan donmedi");
    println!("[faz5-apply] corrupt -> recover(.old) dogrulandi");

    let _ = std::fs::remove_file(&target);
    let _ = std::fs::remove_file(&old);
}

#[tokio::test]
async fn faz5_apply_rejected_service() {
    let _guard = SERIAL.lock().await;
    let mock = spawn_mock_server().await;
    let manifest_url = format!("http://127.0.0.1:{mock}/manifest");
    std::env::set_var("RGSX_UPDATE_MANIFEST_URL", &manifest_url);
    std::env::set_var("RGSX_SELF_APPLY", "1");
    std::env::set_var("RGSX_SERVICE", "1");
    std::env::remove_var("RGSX_SELF_APPLY_TARGET");
    std::env::remove_var("RGSX_SELF_APPLY_TEST");

    let state = AppState::empty();
    let mport = spawn_manager_http(state.clone()).await;
    let events = state.events.clone();
    let sd = state.data.clone();
    tokio::spawn(check_update(events, sd));
    wait_stage(mport, "available", Duration::from_secs(5)).await;

    let c = reqwest::Client::new();
    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/download")).send().await.unwrap();
    let _v = r.json::<Value>().await.unwrap();
    wait_stage(mport, "ready", Duration::from_secs(15)).await;

    let r = c.post(format!("http://127.0.0.1:{mport}/api/manager-update/apply")).send().await.unwrap();
    let v: Value = r.json().await.unwrap();
    assert!(!v["ok"].as_bool().unwrap_or(true), "serviste apply kabul edildi!");
    assert!(v["error"].as_str().unwrap_or("").contains("servis"), "beklenen servis reddi: {:?}", v.get("error"));
    println!("[faz5-apply] serviste apply reddedildi: {:?}", "done");
}
