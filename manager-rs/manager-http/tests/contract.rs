//! Characterization contract testleri — `tests/test_api_contract.py` 1:1.
//!
//! Python altın referansının Rust portu: girdi → (status, headers, payload)
//! birebir eşleşmesi. `tower::ServiceExt::oneshot` ile gerçek axum router'ı
//! çağrılır (soket yok, Python mock handler yaklaşımı gibi).

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

fn has_header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
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
    let (status, _, body) =
        call_post(empty_app(), "/api/download", json!({"url": "https://exemple.invalid/rom.zip", "platform": "NES"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["success"], json!(false));
    assert_eq!(body["error"], json!("Paramètre manquant: game_name requis avec url"));
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
    assert_eq!(body["success"], json!(true));
    assert!(body["ready"].is_boolean());
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