//! Characterization contract testleri — `tests/test_api_contract.py` 1:1.
//!
//! Python altın referansının Rust portu: girdi → (status, headers, payload)
//! birebir eşleşmesi. `tower::ServiceExt::oneshot` ile gerçek axum router'ı
//! çağrılır (soket yok, Python mock handler yaklaşımı gibi).

use std::io::Read;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use manager_http::state::StateData;
use manager_http::{router, AppState};

// ---------------------------------------------------------------------------
// Yardımcılar
// ---------------------------------------------------------------------------

fn empty_app() -> Router {
    router(AppState::empty())
}

fn app_with(data: StateData) -> Router {
    router(AppState::with_data(data, manager_http::sse::channel()))
}

async fn call_get(app: Router, path: &str) -> (StatusCode, Vec<(String, String)>, Value) {
    let res = app
        .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    call_response(res).await
}

async fn call_post(app: Router, path: &str, body: Value) -> (StatusCode, Vec<(String, String)>, Value) {
    let body_bytes = body.to_string();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body_bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    call_response(res).await
}

async fn call_response(
    res: axum::response::Response,
) -> (StatusCode, Vec<(String, String)>, Value) {
    let status = res.status();
    let headers: Vec<(String, String)> = res
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
        .collect();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned()))
    };
    (status, headers, value)
}

async fn call_post_raw(
    app: Router,
    path: &str,
    body: Value,
) -> (StatusCode, Vec<(String, String)>, Vec<u8>) {
    let body_bytes = body.to_string();
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("content-type", "application/json")
                .body(Body::from(body_bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let headers: Vec<(String, String)> = res
        .headers()
        .iter()
        .map(|(k, v)| (k.as_str().to_string(), v.to_str().unwrap_or_default().to_string()))
        .collect();
    let bytes = res.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, bytes)
}

fn has_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// Bridge/placeholder/proxy sözleşme testleri için native DDL yolunu kapatır
/// (`RGSX_NATIVE_DOWNLOAD=0`): isteklerin `native_ddl_download`'a sapmasını önler,
/// böylece non-torrent doğrudan URL'ler bridge/placeholder/katalog-proxy yoluna düşer.
/// Idempotent: değeri "0" yapar; hiçbir test sonunda kaldırmaz (paralel koşumda diğer
/// testleri etkilememek ve env yarışını önlemek için). Tüm bu testler aynı değeri ister.
fn disable_native_download() {
    std::env::set_var("RGSX_NATIVE_DOWNLOAD", "0");
}

// ---------------------------------------------------------------------------
// GET / — page d'accueil
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_root_returns_html() {
    let (status, headers, text) = call_get(empty_app(), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(has_header(&headers, "content-type").unwrap().contains("text/html"));
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(text.contains("RGSX"));
}

// ---------------------------------------------------------------------------
// GET /api/* — formes de reponse
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_platforms_empty() {
    let (status, _, body) = call_get(empty_app(), "/api/platforms").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["count"], json!(0));
    assert_eq!(body["platforms"], json!([]));
}

#[tokio::test]
async fn test_search_empty_query() {
    let (status, _, body) = call_get(empty_app(), "/api/search?q=").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["search_term"], json!(""));
    assert_eq!(body["results"], json!({"platforms": [], "games": []}));
}

#[tokio::test]
async fn test_search_with_term_empty_sources() {
    let (status, _, body) = call_get(empty_app(), "/api/search?q=zelda").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["search_term"], json!("zelda"));
    assert_eq!(body["results"]["platforms"], json!([]));
    assert_eq!(body["results"]["games"], json!([]));
}

#[tokio::test]
async fn test_translations_shape() {
    let (status, _, body) = call_get(empty_app(), "/api/translations").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["language"].is_string());
    assert!(body["translations"].is_object());
}

#[tokio::test]
async fn test_games_empty_platform() {
    let (status, _, body) = call_get(empty_app(), "/api/games/NES").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["platform"], json!("NES"));
    assert_eq!(body["count"], json!(0));
    assert_eq!(body["games"], json!([]));
}

#[tokio::test]
async fn test_progress_empty() {
    let (status, _, body) = call_get(empty_app(), "/api/progress").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["downloads"], json!({}));
}

#[tokio::test]
async fn test_game_status_empty() {
    let (status, _, body) = call_get(empty_app(), "/api/game-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["statuses"], json!({}));
}

#[tokio::test]
async fn test_history_empty() {
    let (status, _, body) = call_get(empty_app(), "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["count"], json!(0));
    assert_eq!(body["history"], json!([]));
}

#[tokio::test]
async fn test_history_strips_error_message_noise() {
    let noisy = "Download error Crazy Cars ++.zip: Accès refusé (HTTP 500). Fichiers disponibles exemples: ['Addams Family.zip', 'After Burner II.zip']";
    let full = noisy.to_string();
    let mut data = StateData::empty();
    data.history = vec![json!({
        "game_name": "Crazy Cars ++.zip",
        "platform": "Amiga OCS ECS (Archive)",
        "status": "Erreur",
        "message": full,
        "url": "https://archive.org/download/amiga-500-Collection/Crazy%20Cars%20%2B%2B.zip",
        "timestamp": "2026-08-11 02:42:48",
    })];

    let (status, _, body) = call_get(app_with(data), "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], json!(1));
    let got = &body["history"][0];
    assert_eq!(got["message"], json!("Accès refusé (HTTP 500)"));
}

#[tokio::test]
async fn test_queue_get() {
    let (status, _, body) = call_get(empty_app(), "/api/queue").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["active"], json!(false));
    assert_eq!(body["queue"], json!([]));
    assert_eq!(body["queue_size"], json!(0));
}

#[tokio::test]
async fn test_settings_get() {
    let (status, _, body) = call_get(empty_app(), "/api/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["settings"].is_object());
}

#[tokio::test]
async fn test_system_info() {
    let (status, _, body) = call_get(empty_app(), "/api/system_info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["system_info"].is_object());
}

#[tokio::test]
async fn test_browse_directories_root() {
    let (status, _, body) = call_get(empty_app(), "/api/browse-directories").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["current_path"].is_string());
    assert!(body["directories"].is_array());
}

#[tokio::test]
async fn test_browse_directories_missing_path() {
    let (status, _, body) = call_get(empty_app(), "/api/browse-directories?path=/chemin/nonexistant-xyz").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Le chemin spécifié n'existe pas"));
}

#[tokio::test]
async fn test_platform_image_not_found() {
    let (status, headers, body) = call_get(empty_app(), "/api/image/NES").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(has_header(&headers, "content-type").unwrap().contains("image/png"));
    assert!(body.is_string());
    assert!(body.as_str().unwrap().contains("PNG"));
}

#[tokio::test]
async fn test_favicon_served() {
    let (status, headers, _) = call_get(empty_app(), "/api/favicon").await;
    assert_eq!(status, StatusCode::OK);
    assert!(has_header(&headers, "content-type").unwrap().contains("image/x-icon"));
}

#[tokio::test]
async fn test_static_missing_file_404() {
    let (status, _, _) = call_get(empty_app(), "/static/js/does_not_exist.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_unknown_route_404() {
    let (status, _, body) = call_get(empty_app(), "/api/inconnue").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Route non trouvée"));
    assert_eq!(body["path"], json!("/api/inconnue"));
}

#[tokio::test]
async fn test_update_cache_no_files() {
    let (status, _, body) = call_get(empty_app(), "/api/update-cache").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["deleted"].is_number());
}

#[tokio::test]
async fn test_cors_header_on_json() {
    let (_, headers, _) = call_get(empty_app(), "/api/platforms").await;
    assert_eq!(has_header(&headers, "access-control-allow-origin"), Some("*"));
}

// ---------------------------------------------------------------------------
// POST /api/* — validation et formes de reponse
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_download_missing_params() {
    let (status, _, body) = call_post(empty_app(), "/api/download", json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(
        body["error"],
        json!("Paramètres manquants: platform et (game_index ou game_name) requis")
    );
}

#[tokio::test]
async fn test_download_invalid_index() {
    let (status, _, body) = call_post(empty_app(), "/api/download", json!({"platform": "NES", "game_index": 0})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("Index de jeu invalide: 0"));
}

#[tokio::test]
async fn test_download_game_name_not_found() {
    let (status, _, body) =
        call_post(empty_app(), "/api/download", json!({"platform": "NES", "game_name": "Introuvable"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], json!("Jeu non trouvé: Introuvable"));
}

#[tokio::test]
async fn test_cancel_missing_url() {
    let (status, _, body) = call_post(empty_app(), "/api/cancel", json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Paramètre manquant: url requis"));
}

#[tokio::test]
async fn test_cancel_unknown_url() {
    let (status, _, body) =
        call_post(empty_app(), "/api/cancel", json!({"url": "https://exemple.invalid/rom.zip"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["message"], json!("Téléchargement annulé"));
    assert_eq!(body["url"], json!("https://exemple.invalid/rom.zip"));
    assert_eq!(body["task_id"], Value::Null);
}

/// Gap-3: bridge varken `/api/cancel` task_id'yi engine'e iletir (catalog yoksa).
#[tokio::test]
async fn test_cancel_with_bridge_forwards_task_id() {
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let bridge: Arc<dyn manager_bridge::TorrentBackend> = Arc::new(FakeCancelEngine {
        calls: calls.clone(),
    });
    let app = app_with_bridge(bridge);

    let (status, _, body) =
        call_post(app, "/api/cancel", json!({"url": "https://exemple.invalid/rom.zip", "task_id": "t1"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["canceled"], json!(true));
    assert_eq!(body["task_id"], json!("t1"));
    assert_eq!(calls.lock().unwrap().as_slice(), &[("cancel_torrent".to_string(), "t1".to_string())]);
}

/// Gap-3: bridge varken `/api/cancel` task_id yoksa `cancel_all`'a düşer.
#[tokio::test]
async fn test_cancel_with_bridge_no_task_forwards_cancel_all() {
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let bridge: Arc<dyn manager_bridge::TorrentBackend> = Arc::new(FakeCancelEngine {
        calls: calls.clone(),
    });
    let app = app_with_bridge(bridge);

    let (status, _, body) =
        call_post(app, "/api/cancel", json!({"url": "https://exemple.invalid/rom.zip"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["canceled"], json!(true));
    assert_eq!(calls.lock().unwrap().as_slice(), &[("cancel_all".to_string(), String::new())]);
}

#[tokio::test]
async fn test_queue_post() {
    let (status, _, body) = call_post(empty_app(), "/api/queue", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["queue_size"], json!(0));
}

#[tokio::test]
async fn test_queue_clear_empty() {
    let (status, _, body) = call_post(empty_app(), "/api/queue/clear", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["cleared_count"], json!(0));
    assert_eq!(body["message"], json!("0 éléments supprimés de la queue"));
}

#[tokio::test]
async fn test_queue_remove_missing_task_id() {
    let (status, _, body) = call_post(empty_app(), "/api/queue/remove", json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Paramètre manquant: task_id requis"));
}

#[tokio::test]
async fn test_queue_remove_not_found() {
    let (status, _, body) = call_post(empty_app(), "/api/queue/remove", json!({"task_id": "xyz"})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Élément non trouvé: xyz"));
}

#[tokio::test]
async fn test_queue_remove_found() {
    let mut data = StateData::empty();
    data.queue = vec![json!({"task_id": "t1", "game_name": "Jeu"})];
    let (status, _, body) = call_post(app_with(data), "/api/queue/remove", json!({"task_id": "t1"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["task_id"], json!("t1"));
}

#[tokio::test]
async fn test_settings_missing_param() {
    let (status, _, body) = call_post(empty_app(), "/api/settings", json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Paramètre \"settings\" manquant"));
}

#[tokio::test]
async fn test_settings_post() {
    let (status, _, body) = call_post(empty_app(), "/api/settings", json!({"settings": {"dummy": 1}})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
}

/// Faz 12f — `RGSX_NATIVE_SETTINGS=1` ile native ayar round-trip'i.
#[tokio::test]
async fn test_settings_native_roundtrip() {
    // Sahte (izole) ayar dosyası — diske bağımlı kalıcı yol yerine TempDir kullanılır.
    // Paralel testlerle çakışmaması ve artık dosya bırakmaması için.
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("rgsx_settings.json");

    // Mevcut env değerlerini koru (paralel koşumda diğer testleri etkilememek için).
    let prev_native = std::env::var("RGSX_NATIVE_SETTINGS").ok();
    let prev_path = std::env::var("RGSX_SETTINGS_PATH").ok();
    std::env::set_var("RGSX_NATIVE_SETTINGS", "1");
    std::env::set_var("RGSX_SETTINGS_PATH", &path);

    let app = empty_app();

    // GET → Python `default_settings` birleşimi.
    let (_, _, body) = call_get(app.clone(), "/api/settings").await;
    assert!(body["success"].as_bool().unwrap_or(false));
    assert_eq!(body["settings"]["language"], json!("en"));
    assert_eq!(body["settings"]["max_simultaneous_downloads"], json!(5));
    assert!(body["system_info"]["system"].is_string());

    // POST geçersiz (invariant ihlali) → 400.
    let (bad_status, _, bad) =
        call_post(app.clone(), "/api/settings", json!({"settings": {"max_simultaneous_downloads": 0}})).await;
    assert_eq!(bad_status, StatusCode::BAD_REQUEST);
    assert!(!bad["success"].as_bool().unwrap_or(true));

    // POST geçerli (language=null → dosyaya yazılmaz).
    let (_, _, ok_body) = call_post(
        app.clone(),
        "/api/settings",
        json!({"settings": {"language": null, "music_enabled": false}}),
    )
    .await;
    assert!(ok_body["success"].as_bool().unwrap_or(false));

    // Kalıcı dosya: language YOK, music_enabled=false, geçici alanlar YOK.
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert!(v.get("language").is_none());
    assert_eq!(v["music_enabled"], json!(false));
    assert!(v.get("api_keys").is_none());

    // Env'i önceki haline döndür (TempDir drop'ta kendi dizinini siler).
    match prev_native {
        Some(v) => std::env::set_var("RGSX_NATIVE_SETTINGS", v),
        None => std::env::remove_var("RGSX_NATIVE_SETTINGS"),
    }
    match prev_path {
        Some(v) => std::env::set_var("RGSX_SETTINGS_PATH", v),
        None => std::env::remove_var("RGSX_SETTINGS_PATH"),
    }
}

#[tokio::test]
async fn test_save_filters() {
    let (status, _, body) = call_post(empty_app(), "/api/save_filters", json!({"region_filters": {}})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["message"], json!("Filtres sauvegardés"));
}

#[tokio::test]
async fn test_clear_history() {
    let (status, _, body) = call_post(empty_app(), "/api/clear-history", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
}

#[tokio::test]
async fn test_restart() {
    let (status, _, body) = call_post(empty_app(), "/api/restart", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["message"], json!("Redémarrage en cours..."));
}

#[tokio::test]
async fn test_post_unknown_route_404() {
    let (status, _, body) = call_post(empty_app(), "/api/inconnue", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["path"], json!("/api/inconnue"));
}

// ---------------------------------------------------------------------------
// Manager (ManagerHandler) endpoints
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health() {
    let (status, _, body) = call_get(empty_app(), "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["status"], json!("ok"));
    assert_eq!(body["manager"], json!(true));
    assert!(body["pid"].as_u64().unwrap() > 0);
    assert!(body["manager_state"].is_string());
}

#[tokio::test]
async fn test_download_direct_url_success() {
    let (status, _, body) = call_post(
        empty_app(),
        "/api/download",
        json!({"url": "https://exemple.invalid/rom.zip", "game_name": "Rom", "platform": "NES"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["queued"], json!(true));
    assert_eq!(body["game_name"], json!("Rom"));
    assert_eq!(body["platform"], json!("NES"));
    assert!(body["task_id"].as_str().unwrap().starts_with("web_"));
}

#[tokio::test]
async fn test_download_direct_url_missing_game_name() {
    disable_native_download();
    let (status, _, body) =
        call_post(empty_app(), "/api/download", json!({"url": "https://exemple.invalid/rom.zip", "platform": "NES"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Paramètre manquant: game_name requis avec url"));
}

// ---------------------------------------------------------------------------
// TASK-002f — /api/download bridge (librqbit engine) proxy
// ---------------------------------------------------------------------------

/// Torrent engine mock'u — `download_torrent` çağrısını kaydeder, anında döner.
#[derive(Debug)]
struct FakeEngine {
    calls: Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

#[async_trait::async_trait]
impl manager_bridge::TorrentBackend for FakeEngine {
    fn engine(&self) -> &'static str {
        "fake"
    }

    async fn call(&self, method: &str, _params: Value) -> Result<Value, manager_bridge::BridgeError> {
        Err(manager_bridge::BridgeError::Rpc {
            code: -32601,
            message: format!("Method not found: {method}"),
        })
    }

    async fn shutdown(&self) {}

    async fn get_app_paths(&self) -> Result<(String, String), manager_bridge::BridgeError> {
        Ok(("/tmp/fake_downloads".to_string(), "/tmp/fake_logs".to_string()))
    }

    async fn download_torrent(
        &self,
        source_url: &str,
        dest_path: &std::path::Path,
        _extract_hint: Option<manager_bridge::ExtractHint>,
    ) -> Result<std::path::PathBuf, manager_bridge::BridgeError> {
        self.calls
            .lock()
            .unwrap()
            .push((source_url.to_string(), dest_path.display().to_string()));
        Ok(dest_path.to_path_buf())
    }
}

/// Gap-3: `cancel_torrent`/`cancel_all` çağrılarını kaydeden test engine.
#[derive(Debug)]
struct FakeCancelEngine {
    calls: Arc<std::sync::Mutex<Vec<(String, String)>>>,
}

#[async_trait::async_trait]
impl manager_bridge::TorrentBackend for FakeCancelEngine {
    fn engine(&self) -> &'static str {
        "fake-cancel"
    }

    async fn call(&self, method: &str, _params: Value) -> Result<Value, manager_bridge::BridgeError> {
        Err(manager_bridge::BridgeError::Rpc {
            code: -32601,
            message: format!("Method not found: {method}"),
        })
    }

    async fn shutdown(&self) {}

    async fn cancel_torrent(&self, task_id: &str) -> Result<bool, manager_bridge::BridgeError> {
        self.calls
            .lock()
            .unwrap()
            .push(("cancel_torrent".to_string(), task_id.to_string()));
        Ok(true)
    }

    async fn cancel_all(&self) -> Result<usize, manager_bridge::BridgeError> {
        self.calls.lock().unwrap().push(("cancel_all".to_string(), String::new()));
        Ok(1)
    }
}

fn app_with_bridge(bridge: Arc<dyn manager_bridge::TorrentBackend>) -> Router {
    router(AppState {
        data: Arc::new(std::sync::RwLock::new(StateData::empty())),
        events: manager_http::sse::channel(),
        bridge: Some(bridge),
        static_root: None,
        catalog: None,
        shutdown: Arc::new(tokio::sync::Notify::new()),
    })
}

#[tokio::test]
async fn test_download_with_bridge_forwards_to_engine() {
    disable_native_download();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let bridge: Arc<dyn manager_bridge::TorrentBackend> =
        Arc::new(FakeEngine { calls: calls.clone() });
    let app = app_with_bridge(bridge);

    let (status, _, body) = call_post(
        app,
        "/api/download",
        json!({"url": "https://exemple.invalid/rom.zip", "game_name": "Rom", "platform": "NES"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["queued"], json!(true));
    assert_eq!(body["game_name"], json!("Rom"));
    assert_eq!(body["platform"], json!("NES"));
    assert!(body["task_id"].as_str().unwrap().starts_with("web_"));

    // Arka plan task'ı engine'i çağırır — kaydı timeout ile bekle.
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let recorded = calls.lock().unwrap().clone();
        if !recorded.is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "engine download_torrent çağrılmadı"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let recorded = calls.lock().unwrap().clone();
    let (url, dest) = recorded.first().expect("en az bir kayıt");
    assert_eq!(url, "https://exemple.invalid/rom.zip");
    // URL basename known ext → engine downloads klasörü altında (path OS-agnostic).
    let expected = std::path::Path::new("/tmp/fake_downloads").join("rom.zip");
    assert_eq!(dest, &expected.display().to_string());
}

/// Hem bridge (librqbit) hem catalog (Python) mevcutken torrent şemalı doğrudan
/// URL'in Python'a proxy EDİLMEDEN engine'e yönlendirilmesi (TASK-002l). Canlıda
/// `catalog` daima vardır; eski kod burada her şeyi Python'a proxy ediyordu.
#[tokio::test]
async fn test_download_torrent_scheme_intercepts_with_bridge_and_catalog() {
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let bridge: Arc<dyn manager_bridge::TorrentBackend> =
        Arc::new(FakeEngine { calls: calls.clone() });

    let mut state = AppState::empty();
    state.bridge = Some(bridge);
    state.catalog = Some(Arc::new(FakeCatalog) as Arc<dyn manager_http::catalog::CatalogSource>);
    let app = router(state);

    let (status, _, body) = call_post(
        app,
        "/api/download",
        json!({"url": "magnet:?xt=urn:btih:abc123", "game_name": "Sintel", "platform": "PC"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["queued"], json!(true));

    // Arka plan task'ı engine'i çağırmalı (yani Python catalog'a DÜŞMEMELİ).
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if !calls.lock().unwrap().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "torrent şemalı istek engine'e yönlenmedi (muhtemelen Python'a proxy edildi)"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let recorded = calls.lock().unwrap().clone();
    assert_eq!(recorded.first().unwrap().0, "magnet:?xt=urn:btih:abc123");
}

/// TASK-002m: canlı progress akışını doğrular. `download_torrent_progress`'u
/// override eden engine, handler'ın verdiği `on_progress` callback'ini çağırır;
/// test engine'in aldığı olayları (downloaded/total) kaydederek callback'in
/// gerçekten çalıştığını ve progress'in state'e işlendiğini kanıtlar.
#[derive(Debug)]
struct FakeProgressEngine {
    events: Arc<std::sync::Mutex<Vec<(u64, u64, f64, bool)>>>,
}

#[async_trait::async_trait]
impl manager_bridge::TorrentBackend for FakeProgressEngine {
    fn engine(&self) -> &'static str {
        "fake-progress"
    }

    async fn call(&self, _method: &str, _params: Value) -> Result<Value, manager_bridge::BridgeError> {
        Err(manager_bridge::BridgeError::Rpc {
            code: -32601,
            message: "Method not found".to_string(),
        })
    }

    async fn shutdown(&self) {}

    async fn download_torrent_progress(
        &self,
        _source_url: &str,
        dest_path: &std::path::Path,
        _task_id: Option<String>,
        on_progress: Option<Arc<dyn Fn(manager_bridge::ProgressEvent) + Send + Sync>>,
        _extract_hint: Option<manager_bridge::ExtractHint>,
    ) -> Result<std::path::PathBuf, manager_bridge::BridgeError> {
        let _ = _task_id;
        if let Some(cb) = &on_progress {
            let ev = manager_bridge::ProgressEvent {
                downloaded: 50,
                total: 100,
                speed: 1.5,
                finished: false,
                paused: false,
            };
            // Engine'in aldığı olayı kaydet (handler'ın callback'inin çalıştığını kanıtlar).
            self.events.lock().unwrap().push((ev.downloaded, ev.total, ev.speed, ev.finished));
            cb(ev);
        }
        Ok(dest_path.to_path_buf())
    }
}

#[tokio::test]
async fn test_download_streams_progress_callback() {
    let events = Arc::new(std::sync::Mutex::new(Vec::new()));
    let engine: Arc<dyn manager_bridge::TorrentBackend> =
        Arc::new(FakeProgressEngine { events: events.clone() });

    let data = Arc::new(std::sync::RwLock::new(StateData::empty()));
    let mut state = AppState::empty();
    state.bridge = Some(engine);
    state.data = data.clone();
    let app = router(state);

    let (status, _, body) = call_post(
        app,
        "/api/download",
        json!({"url": "magnet:?xt=urn:btih:abc", "game_name": "Prog", "platform": "PC"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["queued"], json!(true));

    // Engine'in on_progress callback'ini aldığı (handler'ın callback'inin çalıştığı).
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if !events.lock().unwrap().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "progress callback engine'e ulaşmadı"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let recorded = events.lock().unwrap().clone();
    assert_eq!(recorded.first().unwrap(), &(50u64, 100u64, 1.5f64, false));

    // Sonuçta state.progress tamamlanma (100 / Download_OK) ile sonlanmalı.
    loop {
        let prog = data.read().unwrap().progress.clone();
        if let Some(Value::Object(m)) = prog.get("magnet:?xt=urn:btih:abc") {
            if m.get("status").and_then(Value::as_str) == Some("Download_OK") {
                assert_eq!(m.get("progress").and_then(Value::as_u64), Some(100));
                break;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "indirme finalize olmadı"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

/// Torrent OLMAYAN doğrudan URL (düz http dosya) ve catalog mevcutken hâlâ
/// Python'a proxy edilmeli — engine'e DÜŞMEMELİ (TASK-002l davranış kuralı).
#[tokio::test]
async fn test_download_non_torrent_url_still_proxies_with_catalog() {
    disable_native_download();
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let bridge: Arc<dyn manager_bridge::TorrentBackend> =
        Arc::new(FakeEngine { calls: calls.clone() });

    let mut state = AppState::empty();
    state.bridge = Some(bridge);
    state.catalog = Some(Arc::new(FakeCatalog) as Arc<dyn manager_http::catalog::CatalogSource>);
    let app = router(state);

    let (status, _, body) = call_post(
        app,
        "/api/download",
        json!({"url": "https://exemple.invalid/rom.zip", "game_name": "Rom", "platform": "NES"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Proxy yanıtı: FakeCatalog echo döndürür (engine değil).
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["route"], json!("/api/download"));
    assert!(calls.lock().unwrap().is_empty(), "düz http url engine'e düşmemeli");
}

#[tokio::test]
async fn test_download_bridge_none_keeps_placeholder() {
    disable_native_download();
    let (status, _, body) =
        call_post(empty_app(), "/api/download", json!({"url": "https://exemple.invalid/rom.zip", "game_name": "Rom", "platform": "NES"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["queued"], json!(true));
    assert_eq!(body["message"], json!("Rom ajouté à la file d'attente"));
    assert_eq!(body["queue_position"], json!(1));
}

#[tokio::test]
async fn test_finalize_download_updates_state() {
    let state = AppState::empty();
    {
        let mut data = state.write();
        data.history.push(json!({
            "task_id": "web_123", "game_name": "Rom", "platform": "NES",
            "status": "Queued", "progress": 0,
        }));
        data.queue.push(json!({ "task_id": "web_123", "status": "Queued" }));
    }
    manager_http::api::finalize_download_in_state(
        &state,
        "web_123",
        "https://exemple.invalid/rom.zip",
        "Rom",
        "NES",
        true,
        "/tmp/fake_downloads/rom.zip",
    )
    .await;

    {
        let data = state.read();
        assert_eq!(data.history[0]["status"], json!("Download_OK"));
        assert_eq!(data.history[0]["progress"], json!(100));
        assert!(data.queue.is_empty(), "kuyruk temizlenmeli");
        assert_eq!(data.downloaded["NES"], json!(["Rom"]));
        assert_eq!(data.progress["https://exemple.invalid/rom.zip"]["status"], json!("Download_OK"));
    }

    // Err sonucu status Erreur + downloaded'a eklenmez.
    manager_http::api::finalize_download_in_state(
        &state,
        "web_999",
        "https://exemple.invalid/other.zip",
        "Other",
        "SNES",
        false,
        "Opération impossible",
    )
    .await;
    {
        let data = state.read();
        assert_eq!(data.history[0]["status"], json!("Download_OK"));
        assert!(data.downloaded.get("SNES").is_none(), "hata SNES'e eklenmemeli");
    }
}

#[test]
fn test_dest_path_for_uses_url_basename_when_known_ext() {
    let root = "/tmp/dl";
    assert_eq!(
        manager_http::api::dest_path_for(root, "https://exemple.invalid/rom.zip", "Rom"),
        std::path::PathBuf::from("/tmp/dl/rom.zip")
    );
    assert_eq!(
        manager_http::api::dest_path_for(root, "https://exemple.invalid/console/pack.torrent", "Pack"),
        std::path::PathBuf::from("/tmp/dl/pack.torrent")
    );
}

#[test]
fn test_dest_path_for_falls_back_to_game_name() {
    let root = "/tmp/dl";
    // Magnet URI'nin URL basename'i uzantı taşımaz → game_name.
    let magnet = "magnet:?xt=urn:btih:deadbeef&dn=Some+Game";
    assert_eq!(
        manager_http::api::dest_path_for(root, magnet, "Some Game"),
        std::path::PathBuf::from("/tmp/dl/Some Game")
    );
    // Path ayracı taşıyan oyun adı temizlenir.
    assert_eq!(
        manager_http::api::dest_path_for(root, "https://exemple.invalid/dl", "a/b/c.iso"),
        std::path::PathBuf::from("/tmp/dl/a_b_c.iso")
    );
}

#[tokio::test]
async fn test_shutdown() {
    let (status, _, body) = call_post(empty_app(), "/api/shutdown", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
}

#[tokio::test]
async fn test_pause() {
    let (status, _, body) = call_post(empty_app(), "/api/pause", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["paused"].is_number());
}

#[tokio::test]
async fn test_resume() {
    let (status, _, body) = call_post(empty_app(), "/api/resume", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["resumed"].is_number());
}

#[tokio::test]
async fn test_qbittorrent_start() {
    let (status, _, body) = call_post(empty_app(), "/api/qbittorrent/start", json!({})).await;
    assert_eq!(status, StatusCode::OK);
    // Python 1:1: `success` == `ready` (bridge yoksa ikisi de false).
    assert!(body["ready"].is_boolean());
    assert_eq!(body["success"], body["ready"]);
    assert!(body["url"].is_string());
}

#[tokio::test]
async fn test_qbittorrent_change_password_ok() {
    let (status, _, body) =
        call_post(empty_app(), "/api/qbittorrent/change-password", json!({"password": "nouveau-mdp-123"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["message"], json!("ok"));
}

#[tokio::test]
async fn test_qbittorrent_change_password_failure() {
    let (status, _, body) =
        call_post(empty_app(), "/api/qbittorrent/change-password", json!({"password": "x"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["message"], json!("password_too_short"));
}

#[tokio::test]
async fn test_qbittorrent_password_status() {
    let (status, _, body) = call_get(empty_app(), "/api/qbittorrent/password-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert!(body["available"].is_boolean());
    assert!(body["using_default"].is_boolean());
    assert!(body["webui_url"].is_string());
}

// ---------------------------------------------------------------------------
// SSE
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_sse_event_format() {
    let event = manager_core::contract::sse_event("snapshot", &json!({"active": false}));
    assert!(event.starts_with("event: snapshot\n"));
    assert!(event.contains("data: "));
    let data_part = event.split("data: ").nth(1).unwrap().trim();
    assert_eq!(serde_json::from_str::<Value>(data_part).unwrap(), json!({"active": false}));
    assert!(event.ends_with("\n\n"));
}

#[tokio::test]
async fn test_sse_handler_returns_event_stream_and_snapshot() {
    let app = empty_app();
    let res = app
        .oneshot(Request::builder().uri("/api/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    assert!(
        res.headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap()
            .contains("text/event-stream")
    );
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.starts_with("event: snapshot\n"), "got: {text:?}");
    assert!(text.contains("\"history\"") && text.contains("\"queue\""));
    assert!(text.contains("\"active\"") && text.contains("\"progress\"") && text.contains("\"downloaded\""));
}

#[tokio::test]
async fn test_broadcast_puts_raw_event() {
    let (tx, mut rx) = tokio::sync::broadcast::channel(16);
    manager_http::sse::publish(&tx, "hello", &json!({"x": 1}));
    let raw = rx.recv().await.unwrap();
    assert!(raw.starts_with("event: hello\n"));
    assert!(raw.ends_with("\n\n"));
}

#[test]
fn test_snapshot_has_all_keys() {
    let data = StateData::empty();
    let snap = manager_http::sse::snapshot_json(&data);
    for key in ["history", "queue", "active", "progress", "downloaded"] {
        assert!(snap.as_object().unwrap().contains_key(key), "missing key {key}");
    }
}

#[tokio::test]
async fn debug_alt_syntax() {
    let r = axum::Router::new()
        .route("/api/g/{p}", axum::routing::get(|| async { "ok1" }));
    let res = r
        .oneshot(axum::http::Request::builder().uri("/api/g/NES").body(Body::empty()).unwrap())
        .await
        .unwrap();
    println!("NEW-SYNTAX STATUS: {:?}", res.status());

    let r2 = axum::Router::new()
        .route("/api/g/:p", axum::routing::get(|| async { "ok2" }));
    let res2 = r2
        .oneshot(axum::http::Request::builder().uri("/api/g/NES").body(Body::empty()).unwrap())
        .await
        .unwrap();
    println!("OLD-SYNTAX STATUS: {:?}", res2.status());
}

// ---------------------------------------------------------------------------
// GET /static/* — WebUI statik servisi (TASK-002e)
// ---------------------------------------------------------------------------

/// Geçici static_root kurar; test sonunda silinir. Dizin adı test adına göre
/// benzersizdir (paralel testler çakışmasın).
fn cleanup_static_root(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

fn app_with_static(root: &std::path::Path) -> Router {
    let mut state = AppState::empty();
    state.static_root = Some(root.to_path_buf());
    router(state)
}

fn static_app(unique: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rgsx_static_{unique}"));
    std::fs::create_dir_all(dir.join("js")).unwrap();
    std::fs::create_dir_all(dir.join("css")).unwrap();
    std::fs::write(dir.join("index.html"), "<h1>RGSX</h1>__CSS_VERSION__ __JS_VERSION__").unwrap();
    std::fs::write(dir.join("js/app.js"), "console.log('rgsx');").unwrap();
    std::fs::write(dir.join("css/app.css"), "body{}").unwrap();
    dir
}

#[tokio::test]
async fn test_static_index_served() {
    let dir = static_app("index_served");
    let (status, headers, text) = call_get(app_with_static(&dir), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(has_header(&headers, "content-type").unwrap().contains("text/html"));
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(text.contains("RGSX"));
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_static_index_hydrates_versions() {
    let dir = static_app("index_hydrates");
    let (status, _, text) = call_get(app_with_static(&dir), "/").await;
    assert_eq!(status, StatusCode::OK);
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(!text.contains("__CSS_VERSION__"), "css placeholder hydrate edilmeli");
    assert!(!text.contains("__JS_VERSION__"), "js placeholder hydrate edilmeli");
    assert!(!text.contains("{version}"), "version placeholder hydrate edilmeli");
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_static_js_served() {
    let dir = static_app("js_served");
    let (status, headers, text) = call_get(app_with_static(&dir), "/static/js/app.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(has_header(&headers, "content-type").unwrap().contains("javascript"));
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(text.contains("console.log('rgsx')"));
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_static_css_served() {
    let dir = static_app("css_served");
    let (status, headers, _) = call_get(app_with_static(&dir), "/static/css/app.css").await;
    assert_eq!(status, StatusCode::OK);
    assert!(has_header(&headers, "content-type").unwrap().contains("text/css"));
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_static_missing_returns_404() {
    let dir = static_app("missing_404");
    let (status, _, _) = call_get(app_with_static(&dir), "/static/js/nope.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_static_path_traversal_blocked() {
    let dir = static_app("traversal");
    let (status, _, _) = call_get(app_with_static(&dir), "/static/../secret.txt").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_static_disabled_when_root_none() {
    let (status, _, _) = call_get(empty_app(), "/static/js/app.js").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// SPA fallback — /settings, /downloads, /history, /platform/* → index (Python handlers.py:111)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_spa_settings_serves_index() {
    let dir = static_app("spa_settings");
    let (status, _, text) = call_get(app_with_static(&dir), "/settings").await;
    assert_eq!(status, StatusCode::OK);
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(text.contains("RGSX"), "index içermeli");
    assert!(!text.contains("__CSS_VERSION__"), "gerçek index hydrate edilmiş olmalı (placeholder fallback değil)");
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_spa_downloads_serves_index() {
    let dir = static_app("spa_downloads");
    let (status, _, text) = call_get(app_with_static(&dir), "/downloads").await;
    assert_eq!(status, StatusCode::OK);
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(text.contains("RGSX"));
    assert!(!text.contains("__CSS_VERSION__"), "gerçek index hydrate edilmiş olmalı (placeholder fallback değil)");
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_spa_platform_serves_index() {
    let dir = static_app("spa_platform");
    let (status, _, text) = call_get(app_with_static(&dir), "/platform/NES").await;
    assert_eq!(status, StatusCode::OK);
    let text = match text {
        Value::String(s) => s,
        other => other.to_string(),
    };
    assert!(!text.contains("__CSS_VERSION__"), "gerçek index hydrate edilmiş olmalı (placeholder fallback değil)");
    cleanup_static_root(&dir);
}

#[tokio::test]
async fn test_spa_unknown_non_api_404() {
    let (status, _, _) = call_get(empty_app(), "/bogus/whatever").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
// ---------------------------------------------------------------------------
// Faz 10c/3/2 — katalog proxy (CatalogSource)
// ---------------------------------------------------------------------------

use manager_http::catalog::{CatalogError, CatalogSource};

struct FakeCatalog;

#[async_trait::async_trait]
impl CatalogSource for FakeCatalog {
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError> {
        Ok(json!({
            "success": true,
            "route": route,
            "platforms": ["NES", "SNES"],
            "count": 2,
        }))
    }
    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError> {
        Ok(json!({
            "success": true,
            "route": route,
            "echo": body,
        }))
    }
    async fn post_binary(&self, _route: &str, _body: &Value) -> Result<(Vec<u8>, String), CatalogError> {
        Ok((b"ZIPDATA".to_vec(), "application/zip".to_string()))
    }
    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError> {
        Ok((format!("IMG:{platform}").into_bytes(), "image/png".to_string()))
    }
}

fn app_with_catalog() -> Router {
    let mut state = AppState::empty();
    state.catalog = Some(Arc::new(FakeCatalog) as Arc<dyn CatalogSource>);
    router(state)
}

#[tokio::test]
async fn test_platforms_proxied_birebir() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/platforms").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["route"], json!("/api/platforms"));
    assert_eq!(body["platforms"], json!(["NES", "SNES"]));
}

#[tokio::test]
async fn test_search_proxied_birebir() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/search?q=zelda").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(true));
    assert_eq!(body["route"], json!("/api/search?q=zelda"));
}

#[tokio::test]
async fn test_games_proxied_birebir() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/games/Super%20Nintendo").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/games/Super%20Nintendo"));
}

#[tokio::test]
async fn test_translations_proxied_birebir() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/translations").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/translations"));
}

#[tokio::test]
async fn test_image_proxied_birebir() {
    let (status, headers, body) = call_get(app_with_catalog(), "/api/image/NES").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(has_header(&headers, "content-type"), Some("image/png"));
    let bytes = match body {
        Value::String(s) => s.into_bytes(),
        other => other.to_string().into_bytes(),
    };
    assert_eq!(bytes, b"IMG:NES");
}

#[tokio::test]
async fn test_catalog_placeholder_when_no_source() {
    // catalog None -> eski placeholder davranışı korunur.
    let (status, _, body) = call_get(empty_app(), "/api/platforms").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], json!(0));
    assert_eq!(body["platforms"], json!([]));

    let (status, _, body) = call_get(empty_app(), "/api/search?q=zelda").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["search_term"], json!("zelda"));

    let (status, _, body) = call_get(empty_app(), "/api/games/snes").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["count"], json!(0));
}

// ---------------------------------------------------------------------------
// Faz 10c/3/3 — durum/settings proxy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_settings_get_proxied() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/settings"));
}

#[tokio::test]
async fn test_system_info_proxied() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/system_info").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/system_info"));
}

#[tokio::test]
async fn test_game_status_proxied() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/game-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/game-status"));
}

#[tokio::test]
async fn test_browse_directories_proxied() {
    let (status, _, body) = call_get(app_with_catalog(), "/api/browse-directories?path=/roms").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/browse-directories?path=/roms"));
}

#[tokio::test]
async fn test_settings_post_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/settings", json!({"settings": {"x": 1}})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/settings"));
    assert_eq!(body["echo"]["settings"]["x"], json!(1));
}

#[tokio::test]
async fn test_save_filters_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/save_filters", json!({"a": 1})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/save_filters"));
}

#[tokio::test]
async fn test_status_placeholder_when_no_source() {
    let (status, _, body) = call_get(empty_app(), "/api/settings").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["success"].as_bool().unwrap_or(false));
    let (status, _, body) = call_get(empty_app(), "/api/system_info").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, body) = call_get(empty_app(), "/api/game-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["statuses"], json!({}));
    let (status, _, body) = call_get(empty_app(), "/api/browse-directories?path=/nope").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

// ---------------------------------------------------------------------------
// Faz 10c/3/4 — destek/queue yönetimi proxy
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cancel_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/cancel", json!({"url": "x"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/cancel"));
}

#[tokio::test]
async fn test_queue_post_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/queue", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/queue"));
}

#[tokio::test]
async fn test_queue_clear_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/queue/clear", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/queue/clear"));
}

#[tokio::test]
async fn test_queue_remove_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/queue/remove", json!({"task_id": "t1"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/queue/remove"));
}

#[tokio::test]
async fn test_clear_history_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/clear-history", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/clear-history"));
}

#[tokio::test]
async fn test_restart_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/restart", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/restart"));
}

#[tokio::test]
async fn test_shutdown_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/shutdown", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/shutdown"));
}

#[tokio::test]
async fn test_pause_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/pause", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/pause"));
}

#[tokio::test]
async fn test_resume_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/resume", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/resume"));
}

#[tokio::test]
async fn test_support_proxied_binary() {
    let (status, headers, body) = call_post(app_with_catalog(), "/api/support", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(has_header(&headers, "content-type"), Some("application/zip"));
    let bytes = match body {
        Value::String(s) => s.into_bytes(),
        other => other.to_string().into_bytes(),
    };
    assert_eq!(bytes, b"ZIPDATA");
}

#[tokio::test]
async fn test_support_redacted_offline() {
    // TASK-002-gap-13: catalog-off modda üretilen ZIP, gizli alanları redakte etmeli.
    let mut data = StateData::empty();
    data.settings = json!({
        "language": "tr",
        "qbittorrent_webui_password": "s3cret!",
        "sources": { "mode": "rgsx", "custom_url": "https://example.com", "api_key": "k123" }
    });
    let app = app_with(data);
    let (status, headers, bytes) = call_post_raw(app, "/api/support", Value::Null).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        has_header(&headers, "content-type"),
        Some("application/zip")
    );
    assert!(!bytes.is_empty(), "boş zip olmamalı");

    // ZIP'i parse et, rgsx_settings.json içeriğini kontrol et.
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("geçerli zip olmalı");
    let names: Vec<String> = zip.file_names().map(|n| n.to_string()).collect();
    assert!(names.iter().any(|n| n == "rgsx_settings.json"));
    assert!(names.iter().any(|n| n == "README.txt"));

    let mut settings_text = String::new();
    zip.by_name("rgsx_settings.json")
        .expect("rgsx_settings.json bulunmalı")
        .read_to_string(&mut settings_text)
        .unwrap();
    let parsed: Value = serde_json::from_str(&settings_text).expect("geçerli JSON olmalı");

    assert_eq!(parsed["qbittorrent_webui_password"], json!("<redacted>"));
    assert_eq!(parsed["sources"]["api_key"], json!("<redacted>"));
    // hassas olmayanlar dokunulmaz.
    assert_eq!(parsed["language"], json!("tr"));
    assert_eq!(parsed["sources"]["mode"], json!("rgsx"));
    assert_eq!(parsed["sources"]["custom_url"], json!("https://example.com"));
    // ham şifre/key ZIP içinde görünmemeli.
    assert!(!settings_text.contains("s3cret!"));
    assert!(!settings_text.contains("k123"));
}

#[tokio::test]
async fn test_clear_history_preserves_active() {
    // TASK-002-gap-10 (B): clear_history aktif indirmeyi korumalı, biteni silmeli.
    let mut data = StateData::empty();
    data.history = json!([
        { "game_name": "Active", "status": "Downloading", "url": "http://x/active", "task_id": "t-active" },
        { "game_name": "Done", "status": "Download_OK", "url": "http://x/done", "task_id": "t-done" }
    ])
    .as_array()
    .unwrap()
    .clone();
    data.queue = json!([
        { "url": "http://x/active", "task_id": "t-active", "status": "Queued" }
    ])
    .as_array()
    .unwrap()
    .clone();

    // AppState'i clone tutup clear sonrası state'i doğrudan inceleyelim
    // (call_post Router'ı tüketir; Arc aynı kaldığı için `st` ile okuruz).
    let st = AppState::with_data(data, manager_http::sse::channel());
    let app = router(st.clone());
    let (status, _, _) = call_post(app, "/api/clear-history", Value::Null).await;
    assert_eq!(status, StatusCode::OK);

    let remaining = st.read().history.clone();
    assert_eq!(remaining.len(), 1, "yalnızca aktif entry kalmalı");
    assert_eq!(remaining[0]["game_name"], json!("Active"));
    assert_eq!(remaining[0]["status"], json!("Downloading"));
}

#[tokio::test]
async fn test_download_batch_proxied() {
    let (status, _, body) = call_post(app_with_catalog(), "/api/download/batch", json!({"games": []})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["route"], json!("/api/download/batch"));
}

#[tokio::test]
async fn test_download_batch_disabled_without_catalog() {
    let (status, _, _) = call_post(empty_app(), "/api/download/batch", json!({"games": []})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_mgmt_placeholder_when_no_source() {
    let (status, _, body) = call_post(empty_app(), "/api/pause", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["paused"].is_number());
    let (status, _, body) = call_post(empty_app(), "/api/resume", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["resumed"], json!(0));
}

// ---------------------------------------------------------------------------
// Faz 10c/3/5 — qBittorrent bridge (TorrentBackend) handler'ları
// Not: bu handler'lar zaten `state.bridge_call` ile TorrentBackend trait'ine
// bağlı; köprü (bridge) yoksa placeholder'a düşer (geriye uyumlu).
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_qb_change_password_short_fails() {
    let (status, _, body) = call_post(empty_app(), "/api/qbittorrent/change-password", json!({"password": "x"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(!body["success"].as_bool().unwrap_or(true));
    assert_eq!(body["message"], json!("password_too_short"));
}

#[tokio::test]
async fn test_qb_change_password_ok_placeholder() {
    let (status, _, body) = call_post(empty_app(), "/api/qbittorrent/change-password", json!({"password": "longenough"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], json!("ok"));
}

#[tokio::test]
async fn test_qb_start_placeholder() {
    let (status, _, body) = call_post(empty_app(), "/api/qbittorrent/start", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["ready"], json!(false));
    assert_eq!(body["url"], json!(""));
}

#[tokio::test]
async fn test_qb_password_status_placeholder() {
    let (status, _, body) = call_get(empty_app(), "/api/qbittorrent/password-status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["available"], json!(false));
    assert_eq!(body["using_default"], json!(true));
    assert_eq!(body["webui_url"], json!(""));
}

#[tokio::test]
async fn test_qb_regenerate_password_bridge_unavailable() {
    // Köprü yok → 500 + birebir Python sözleşmesi (success:false, message).
    let (status, _, body) = call_post(empty_app(), "/api/qbittorrent/regenerate-password", Value::Null).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["success"], json!(false));
    assert!(body["message"].as_str().is_some());
}

// ---------------------------------------------------------------------------
// TASK-002-gap-1: job-level retry motoru contract testleri
// ---------------------------------------------------------------------------

/// İlk `fail_times` çağrıda transient (`Timeout`) hata döner, sonra başarılı olur.
#[derive(Debug)]
struct FlakyEngine {
    calls: Arc<std::sync::Mutex<Vec<String>>>,
    fail_times: usize,
}

#[async_trait::async_trait]
impl manager_bridge::TorrentBackend for FlakyEngine {
    fn engine(&self) -> &'static str {
        "flaky"
    }
    async fn call(&self, _method: &str, _params: Value) -> Result<Value, manager_bridge::BridgeError> {
        Err(manager_bridge::BridgeError::Rpc {
            code: -32601,
            message: "n/a".into(),
        })
    }
    async fn shutdown(&self) {}
    async fn download_torrent_progress(
        &self,
        _source_url: &str,
        dest_path: &std::path::Path,
        _task_id: Option<String>,
        _on_progress: Option<Arc<dyn Fn(manager_bridge::ProgressEvent) + Send + Sync>>,
        _extract_hint: Option<manager_bridge::ExtractHint>,
    ) -> Result<std::path::PathBuf, manager_bridge::BridgeError> {
        let n = self.calls.lock().unwrap().len();
        self.calls.lock().unwrap().push("download".to_string());
        if n < self.fail_times {
            return Err(manager_bridge::BridgeError::Timeout(
                "geçici ağ hatası".into(),
            ));
        }
        Ok(dest_path.to_path_buf())
    }
}

#[tokio::test]
async fn test_download_retries_then_succeeds() {
    let calls = Arc::new(std::sync::Mutex::new(Vec::new()));
    let engine: Arc<dyn manager_bridge::TorrentBackend> =
        Arc::new(FlakyEngine {
            calls: calls.clone(),
            fail_times: 2,
        });
    let data = Arc::new(std::sync::RwLock::new(StateData::empty()));
    let mut state = AppState::empty();
    state.bridge = Some(engine);
    state.data = data.clone();
    let app = router(state);

    let (status, _, body) = call_post(
        app,
        "/api/download",
        json!({"url": "magnet:?xt=urn:btih:abc123", "game_name": "Rom", "platform": "NES"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["queued"], json!(true));

    // 1 başlangıç + 2 retry = 3 engine çağrısı (fail_times=2 → 3. çağrı başarı).
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if calls.lock().unwrap().len() >= 3 {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "retry motoru beklenen çağrı sayısına ulaşmadı"
        );
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let entries = data.read().unwrap().history.clone();
    // 1 initial + 2 retry = 3 entry (son deneme başarılı → yeni retry girişi açılmaz).
    assert_eq!(entries.len(), 3, "retry sayısı (3 deneme) 3 history entry üretmeli");
    let task_ids: Vec<&str> = entries
        .iter()
        .map(|e| e["task_id"].as_str().unwrap())
        .collect();
    let unique: std::collections::HashSet<&str> = task_ids.iter().copied().collect();
    assert_eq!(
        unique.len(),
        3,
        "her deneme yeni task_id kullanmalı (Python queue.py:610 parity)"
    );
    // İlk iki entry transient başarısızlık sonrası RETRY_SCHEDULED olmalı.
    assert_eq!(entries[0]["entity_state"], json!("RETRY_SCHEDULED"));
    assert_eq!(entries[0]["retry_count"], json!(1));
    assert_eq!(entries[1]["retry_count"], json!(2));
    // Son entry başarılı → COMPLETED.
    assert_eq!(entries[2]["entity_state"], json!("COMPLETED"));
    assert_eq!(entries[2]["status"], json!("Download_OK"));
}

#[tokio::test]
async fn test_history_includes_retry_fields_additive() {
    // Mevcut sözleşme bozulmadan retry alanları eklendi (backward-compat).
    let data = StateData {
        history: vec![json!({
            "game_name": "Rom",
            "platform": "NES",
            "url": "https://e.invalid/r.zip",
            "status": "Download_OK",
            "progress": 100,
            "message": "Tamamlandı",
            "timestamp": "",
            "downloaded_size": 0,
            "total_size": 0,
            "task_id": "web_abc",
            "entity_state": "COMPLETED",
            "retry_count": 2,
            "max_retries": 3,
            "retry_at": 0,
        })],
        ..StateData::empty()
    };
    let app = app_with(data);
    let (status, _, body) = call_get(app, "/api/history").await;
    assert_eq!(status, StatusCode::OK);
    let arr = body["history"].as_array().unwrap();
    assert_eq!(arr.len(), 1);
    let e = &arr[0];
    // Temel sözleşme alanları korunur.
    assert_eq!(e["game_name"], json!("Rom"));
    assert_eq!(e["status"], json!("Download_OK"));
    assert_eq!(e["task_id"], json!("web_abc"));
    // Yeni additive alanlar mevcut.
    assert_eq!(e["entity_state"], json!("COMPLETED"));
    assert_eq!(e["retry_count"], json!(2));
    assert_eq!(e["max_retries"], json!(3));
    assert_eq!(e["retry_at"], json!(0));
}
