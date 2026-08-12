//! `/api/*` route handler'ları + yanıt şablonları.
//!
//! TASK-002b — Python `RGSXHandler`/`ManagerHandler` sözleşmesi 1:1
//! (`tests/test_api_contract.py` altın referansı). İş mantığı placeholder:
//! gerçek download/queue/settings eylemleri TASK-002c bridge entegrasyonunda.
//!
//! Yanıt kuralları (handlers.py `_set_headers`/`_send_json`):
//! - Her yanıtta `Access-Control-Allow-Origin: *`
//! - Başarı: `{"success": true, ...}` ; Hata: `{"success": false, "error": msg}`

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};

use manager_core::contract;
use manager_core::state::ManagerState;

use crate::sse;
use crate::state::AppState;

/// CORS başlığı (tüm yanıtlarda; handlers.py `_set_headers`).
const CORS: [(&str, &str); 1] = [("Access-Control-Allow-Origin", "*")];

fn cors_response(status: StatusCode, value: Value) -> Response {
    (status, CORS, Json(value)).into_response()
}

pub fn ok(value: Value) -> Response {
    cors_response(StatusCode::OK, value)
}

pub fn json_err(msg: impl Into<String>, status: StatusCode) -> Response {
    cors_response(status, contract::error(msg))
}

/// `task_id` için milisaniye stamp (`f"web_{int(time.time()*1000)}"`).
fn web_task_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("web_{millis}")
}

// ---------------------------------------------------------------------------
// GET
// ---------------------------------------------------------------------------

/// GET `/` — index sayfası (`static_root/index.html`; yoksa minimal placeholder).
pub async fn index(State(state): State<AppState>) -> Response {
    let root = match &state.static_root {
        Some(r) => r.clone(),
        None => return placeholder_index(),
    };
    let index_path = root.join("index.html");
    if let Ok(html) = std::fs::read_to_string(&index_path) {
        let html = hydrate_index(&html, &root);
        return (StatusCode::OK, [("Content-Type", "text/html; charset=utf-8")], html).into_response();
    }
    placeholder_index()
}

/// Placeholder index — static_root yoksa veya index.html okunamazsa.
fn placeholder_index() -> Response {
    let html = "<!doctype html><html><head><title>RGSX Manager</title></head>\
                <body><h1>RGSX Manager</h1></body></html>";
    (StatusCode::OK, [("Content-Type", "text/html; charset=utf-8")], html).into_response()
}

/// `__CSS_VERSION__`/`__JS_VERSION__` placeholder'larını asset mtime'larına,
/// `{version}`'u uygulama versiyonuna göre doldurur.
fn hydrate_index(html: &str, static_root: &std::path::Path) -> String {
    let css_version = asset_version(static_root, "css/app.css");
    let js_version = asset_version(static_root, "js/app.js");
    let version = std::env::var("RGSX_MANAGER_VERSION").unwrap_or_else(|_| "0.1.0".to_string());
    html.replace("__CSS_VERSION__", &css_version)
        .replace("__JS_VERSION__", &js_version)
        .replace("{version}", &version)
}

/// Asset dosyasının mtime'unu saniye olarak döndürür; hata durumunda boş string.
fn asset_version(static_root: &std::path::Path, relative: &str) -> String {
    std::fs::metadata(static_root.join(relative))
        .and_then(|m| m.modified())
        .map(|t| t.duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0).to_string())
        .unwrap_or_default()
}

/// GET `/static/{*path}` — WebUI statik dosyaları (path traversal korumalı).
pub async fn static_file(
    State(state): State<AppState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    let root = match &state.static_root {
        Some(r) => r.clone(),
        None => return (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    };
    // Normalleştir; `..` geçişini reddet.
    let safe: std::path::PathBuf = path.split('/').collect();
    if safe.components().any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::RootDir)) {
        return (StatusCode::NOT_FOUND, "404 Not Found").into_response();
    }
    let file_path = root.join(&safe);
    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let mime = mime_for(&file_path);
            (StatusCode::OK, [("Content-Type", mime), ("Cache-Control", "public, max-age=3600")], bytes).into_response()
        }
        Err(_) => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
    }
}

/// Basit MIME tespiti (genişletilebilir).
fn mime_for(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "js" => "application/javascript",
        "css" => "text/css",
        "json" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "woff" | "woff2" => "font/woff2",
        "html" => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// GET `/api/platforms` — boş liste placeholder.
pub async fn platforms() -> Response {
    ok(contract::ok(json!({ "count": 0, "platforms": [] })))
}

/// GET `/api/search?q=...` — boş sonuç, search_term yansıtılır.
pub async fn search(Query(params): Query<std::collections::HashMap<String, String>>) -> Response {
    let term = params.get("q").cloned().unwrap_or_default();
    ok(contract::ok(json!({
        "search_term": term,
        "results": { "platforms": [], "games": [] },
    })))
}

/// GET `/api/translations` — placeholder: `language` + `translations._language`.
pub async fn translations() -> Response {
    ok(contract::ok(json!({
        "language": "tr",
        "translations": { "_language": "tr" },
    })))
}

/// GET `/api/games/{platform}` — boş liste placeholder.
pub async fn games(AxumPath(platform): AxumPath<String>) -> Response {
    ok(contract::ok(json!({
        "platform": platform,
        "count": 0,
        "games": [],
    })))
}

/// GET `/api/progress` — `config.download_progress` eşleniği.
pub async fn progress(State(state): State<AppState>) -> Response {
    let downloads = state.read().progress.clone();
    ok(contract::ok(json!({ "downloads": downloads })))
}

/// GET `/api/game-status` — boş `statuses` placeholder.
pub async fn game_status() -> Response {
    ok(contract::ok(json!({ "statuses": {} })))
}

/// GET `/api/history` — history + message noise stripping.
pub async fn history(State(state): State<AppState>) -> Response {
    let history = state.read().history.clone();
    let cleaned: Vec<Value> = history
        .into_iter()
        .map(|mut entry| {
            if let Some(Value::String(msg)) = entry.get_mut("message") {
                *msg = contract::strip_history_error_noise(msg);
            }
            entry
        })
        .collect();
    ok(contract::ok(json!({ "count": cleaned.len(), "history": cleaned })))
}

/// GET `/api/queue` — kuyruk durumu.
pub async fn queue(State(state): State<AppState>) -> Response {
    let data = state.read();
    ok(contract::ok(json!({
        "active": data.active,
        "queue": data.queue,
        "queue_size": data.queue_size(),
    })))
}

/// GET `/api/settings` — placeholder ayar (iş mantığı TASK-002c).
pub async fn settings_get(State(state): State<AppState>) -> Response {
    let settings = state.read().settings.clone();
    ok(contract::ok(json!({ "settings": settings })))
}

/// GET `/api/system_info` — placeholder.
pub async fn system_info(State(state): State<AppState>) -> Response {
    let info = state.read().system_info.clone();
    ok(contract::ok(json!({ "system_info": info })))
}

/// GET `/api/browse-directories?path=...` — path yoksa 400 (Python birebir).
pub async fn browse_directories(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let requested = params.get("path");
    if let Some(path) = requested {
        if !std::path::Path::new(path).exists() {
            return json_err("Le chemin spécifié n'existe pas", StatusCode::BAD_REQUEST);
        }
    }
    let base = requested.cloned().unwrap_or_default();
    let (current, dirs) = state.read().browse(&base);
    ok(contract::ok(json!({ "current_path": current, "directories": dirs })))
}

/// GET `/api/image/{platform}` — 404 `image/png` + PNG başlığı (placeholder).
pub async fn image() -> Response {
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    (
        StatusCode::NOT_FOUND,
        [("Content-Type", "image/png"), ("Access-Control-Allow-Origin", "*")],
        PNG,
    )
        .into_response()
}

/// GET `/api/favicon` — 200 `image/x-icon` (placeholder).
pub async fn favicon() -> Response {
    (
        StatusCode::OK,
        [("Content-Type", "image/x-icon"), ("Access-Control-Allow-Origin", "*")],
        b"\x00\x00\x01\x00".as_slice(),
    )
        .into_response()
}

/// GET `/api/update-cache` — placeholder `deleted: 0`.
pub async fn update_cache() -> Response {
    ok(contract::ok(json!({ "deleted": 0 })))
}

// ---------------------------------------------------------------------------
// POST — web
// ---------------------------------------------------------------------------

/// POST `/api/download` — doğrulama sırası Python `_handle_download_worker`
/// (rgsx_manager.py:337) ile birebir; başarı placeholder queue push.
pub async fn download(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let platform = body.get("platform").and_then(Value::as_str);
    let game_index = body.get("game_index");
    let game_name = body.get("game_name").and_then(Value::as_str);
    let direct_url = body.get("url").and_then(Value::as_str);

    if platform.is_none() || (game_index.is_none() && game_name.is_none() && direct_url.is_none()) {
        return json_err(
            "Paramètres manquants: platform et (game_index ou game_name) requis",
            StatusCode::BAD_REQUEST,
        );
    }

    let game_url;
    if let Some(direct) = direct_url {
        let Some(gname) = game_name else {
            return json_err(
                "Paramètre manquant: game_name requis avec url",
                StatusCode::BAD_REQUEST,
            );
        };
        game_url = direct.to_string();
        let task_id = web_task_id();
        let entry = json!({
            "url": game_url,
            "platform": platform.unwrap(),
            "game_name": gname,
            "task_id": task_id,
            "status": "Queued",
        });
        let mut data = state.write();
        data.queue.push(entry.clone());
        data.history.push(json!({
            "game_name": gname,
            "platform": platform.unwrap(),
            "url": game_url,
            "status": "Queued",
            "timestamp": "",
        }));
        sse::publish(
            &state.events,
            "queue",
            &json!({ "queue": data.queue, "active": data.active }),
        );
        drop(data);
        return ok(contract::ok(json!({
            "queued": true,
            "game_name": gname,
            "platform": platform.unwrap(),
            "task_id": task_id,
        })));
    }

    // Katalog placeholder boş → game_index/game_name asla bulunamaz (TASK-002c).
    if let Some(name) = game_name {
        return json_err(format!("Jeu non trouvé: {name}"), StatusCode::BAD_REQUEST);
    }
    let idx = game_index.and_then(Value::as_i64).unwrap_or(-1);
    json_err(format!("Index de jeu invalide: {idx}"), StatusCode::BAD_REQUEST)
}

/// POST `/api/cancel` — 400 `url` eksikse; yoksa placeholder 200 (task_id: null).
pub async fn cancel(Json(body): Json<Value>) -> Response {
    let Some(url) = body.get("url").and_then(Value::as_str) else {
        return json_err("Paramètre manquant: url requis", StatusCode::BAD_REQUEST);
    };
    ok(contract::ok(json!({
        "message": "Téléchargement annulé",
        "url": url,
        "task_id": Value::Null,
    })))
}

/// POST `/api/queue` — `queue_size` döner.
pub async fn queue_post(State(state): State<AppState>) -> Response {
    let size = state.read().queue_size();
    ok(contract::ok(json!({ "queue_size": size })))
}

/// POST `/api/queue/clear` — kuyruğu boşaltır.
pub async fn queue_clear(State(state): State<AppState>) -> Response {
    let mut data = state.write();
    let cleared = data.queue.len();
    data.queue.clear();
    sse::publish(
        &state.events,
        "queue",
        &json!({ "queue": data.queue, "active": data.active }),
    );
    ok(contract::ok(json!({
        "cleared_count": cleared,
        "message": format!("{cleared} éléments supprimés de la queue"),
    })))
}

/// POST `/api/queue/remove` — task_id eksikse 400, yoksa 404, varsa 200 + kaldır.
pub async fn queue_remove(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let Some(task_id) = body.get("task_id").and_then(Value::as_str) else {
        return json_err("Paramètre manquant: task_id requis", StatusCode::BAD_REQUEST);
    };
    let task_id = task_id.to_string();
    let mut data = state.write();
    if let Some(pos) = data
        .queue
        .iter()
        .position(|e| e.get("task_id").and_then(Value::as_str) == Some(task_id.as_str()))
    {
        data.queue.remove(pos);
        sse::publish(
            &state.events,
            "queue",
            &json!({ "queue": data.queue, "active": data.active }),
        );
        return ok(contract::ok(json!({ "task_id": task_id })));
    }
    json_err(format!("Élément non trouvé: {task_id}"), StatusCode::NOT_FOUND)
}

/// POST `/api/settings` — "settings" anahtarı zorunlu; placeholder.
pub async fn settings_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    let Some(settings) = body.get("settings") else {
        return json_err("Paramètre \"settings\" manquant", StatusCode::BAD_REQUEST);
    };
    state.write().settings = settings.clone();
    ok(contract::ok(Value::Null))
}

/// POST `/api/save_filters` — placeholder.
pub async fn save_filters() -> Response {
    ok(contract::ok(json!({ "message": "Filtres sauvegardés" })))
}

/// POST `/api/clear-history` — geçmişi temizler.
pub async fn clear_history(State(state): State<AppState>) -> Response {
    state.write().history.clear();
    ok(contract::ok(Value::Null))
}

/// POST `/api/restart` — placeholder mesaj.
pub async fn restart() -> Response {
    ok(contract::ok(json!({ "message": "Redémarrage en cours..." })))
}

/// POST `/api/support` — placeholder zip (iş mantığı TASK-002c).
pub async fn support() -> Response {
    (
        StatusCode::OK,
        [
            ("Content-Type", "application/zip"),
            ("Access-Control-Allow-Origin", "*"),
            ("Content-Disposition", "attachment; filename=rgsx_support.zip"),
        ],
        b"".as_slice(),
    )
        .into_response()
}

// ---------------------------------------------------------------------------
// POST / GET — manager
// ---------------------------------------------------------------------------

/// GET `/api/health` — `pid>0`, `manager_state` string.
pub async fn health(State(state): State<AppState>) -> Response {
    let data = state.read();
    ok(contract::ok(json!({
        "status": "ok",
        "manager": true,
        "pid": data.pid,
        "manager_state": data.manager_state.to_string(),
    })))
}

/// POST `/api/shutdown` — placeholder.
pub async fn shutdown() -> Response {
    ok(contract::ok(Value::Null))
}

/// POST `/api/pause` — aktif kuyruk elemanı sayısı (placeholder).
pub async fn pause(State(state): State<AppState>) -> Response {
    let paused = state.read().queue_size();
    ok(contract::ok(json!({ "paused": paused })))
}

/// POST `/api/resume` — placeholder (0).
pub async fn resume() -> Response {
    ok(contract::ok(json!({ "resumed": 0 })))
}

/// POST `/api/qbittorrent/change-password` — bridge'e `change_webui_password`.
/// Python 1:1: `(ok, message)` → başarı `{"message":"ok"}`, başarısız 400.
pub async fn change_password(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Map<String, Value>>,
) -> Response {
    let Some(pw) = body.get("password").and_then(Value::as_str) else {
        return json_err("Paramètre manquant: password requis", StatusCode::BAD_REQUEST);
    };
    match state
        .bridge_call("change_webui_password", json!({ "password": pw }))
        .await
    {
        Ok(v) => {
            let ok_flag = v.get(0).and_then(Value::as_bool).unwrap_or(false);
            let msg = v.get(1).and_then(Value::as_str).unwrap_or_default();
            if !ok_flag {
                return cors_response(
                    StatusCode::BAD_REQUEST,
                    json!({ "success": false, "message": msg }),
                );
            }
            ok(contract::ok(json!({ "message": "ok" })))
        }
        Err(_) => {
            // bridge yok/çökük — placeholder politikası (contract: uzunluk < 8 → 400).
            if pw.len() < 8 {
                return cors_response(
                    StatusCode::BAD_REQUEST,
                    json!({ "success": false, "message": "password_too_short" }),
                );
            }
            ok(contract::ok(json!({ "message": "ok" })))
        }
    }
}

/// POST `/api/qbittorrent/start` — bridge `ensure_running` + `get_webui_url`.
/// Python 1:1: `{"success": ready, "ready": ready, "url": url}`.
pub async fn qb_start(State(state): State<AppState>) -> Response {
    let ready = state
        .bridge_call("ensure_running", json!({ "timeout": 30.0 }))
        .await
        .map(|v| v.as_bool().unwrap_or(false))
        .unwrap_or(false);
    let url = state
        .bridge_call("get_webui_url", json!({}))
        .await
        .map(|u| u.as_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    ok(contract::ok(json!({ "success": ready, "ready": ready, "url": url })))
}

/// GET `/api/qbittorrent/password-status` — bridge `get_password_status`.
/// Python 1:1: `{"success": True, **status}`.
pub async fn qb_password_status(State(state): State<AppState>) -> Response {
    match state.bridge_call("get_password_status", json!({})).await {
        Ok(v) => {
            let mut body = json!({ "success": true });
            if let Some(obj) = v.as_object() {
                for (k, val) in obj {
                    body[k] = val.clone();
                }
            }
            ok(body)
        }
        Err(_) => ok(contract::ok(json!({
            "available": false,
            "using_default": true,
            "webui_url": "",
        }))),
    }
}

// ---------------------------------------------------------------------------
// Fallback
// ---------------------------------------------------------------------------

/// 404 — `/api/*`'de JSON `Route non trouvée` + path; SPA yollarında index;
/// diğer yollarda düz 404.
///
/// Not: `Request` body tüketen tail extractor olduğundan son parametre olmalı;
/// `State` `FromRequestParts` olduğundan önce gelir.
pub async fn fallback(State(state): State<AppState>, req: axum::extract::Request) -> Response {
    let path = req.uri().path().to_string();
    if path.starts_with("/api/") {
        let mut body = contract::error("Route non trouvée");
        if let Value::Object(map) = &mut body {
            map.insert("path".to_string(), Value::String(path));
        }
        return cors_response(StatusCode::NOT_FOUND, body);
    }
    // SPA yolları — Python handlers.py:111 (`/settings`, `/downloads`, ...)
    // tarayıcıda index'ten tab'ı açar; index servis edilir.
    if is_spa_path(&path) {
        return index(State(state)).await;
    }
    (StatusCode::NOT_FOUND, [("Access-Control-Allow-Origin", "*")], "404 Not Found").into_response()
}

/// Tepsi/Açma navigasyon yolları (`/`, `/index.html`, `/platform/`, `/settings`,
/// `/downloads`, `/history`).
fn is_spa_path(path: &str) -> bool {
    path == "/"
        || path == "/index.html"
        || path == "/downloads"
        || path == "/history"
        || path == "/settings"
        || path.starts_with("/platform/")
}

// ---------------------------------------------------------------------------
// Yerel tipler
// ---------------------------------------------------------------------------

/// `data.pid` erişicisi (test yardımcısı).
pub fn pid_of(state: &AppState) -> u32 {
    state.read().pid
}

/// `manager_state` erişicisi (test yardımcısı).
pub fn manager_state_of(state: &AppState) -> ManagerState {
    state.read().manager_state
}

/// Test: Sabit durumdaki `pid` için.
pub fn set_pid(state: &AppState, pid: u32) {
    state.write().pid = pid;
}

/// Paylaşılan `Arc` erişimi — handler katmanında iş mantığı eklentisi için.
#[allow(dead_code)]
pub fn data_arc(state: &AppState) -> Arc<std::sync::RwLock<crate::state::StateData>> {
    Arc::clone(&state.data)
}