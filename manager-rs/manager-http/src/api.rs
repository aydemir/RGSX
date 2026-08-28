//! `/api/*` route handler'ları + yanıt şablonları.
//!
//! TASK-002b — Python `RGSXHandler`/`ManagerHandler` sözleşmesi 1:1
//! (`tests/test_api_contract.py` altın referansı). İş mantığı placeholder:
//! gerçek download/queue/settings eylemleri TASK-002c bridge entegrasyonunda.
//!
//! Yanıt kuralları (handlers.py `_set_headers`/`_send_json`):
//! - Her yanıtta `Access-Control-Allow-Origin: *`
//! - Başarı: `{"success": true, ...}` ; Hata: `{"success": false, "error": msg}`

use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio::sync::{mpsc, Notify};

use manager_bridge::BridgeError;
use manager_core::contract;
use manager_core::retry::{self, ErrorClass};
use manager_core::secrets::redact_secrets;
use manager_core::state::ManagerState;
use manager_download::http::stream::CancelFlag;
use manager_download::http::DownloadError;

use crate::state::{AppState, QueueCommand, QueueStatus, QueuedItem, TaskState};

/// CORS başlığı (tüm yanıtlarda; handlers.py `_set_headers`).
const CORS: [(&str, &str); 1] = [("Access-Control-Allow-Origin", "*")];

/// TASK-002-gap-32: ardışık ağ hatası eşiği. Bu sayıda üst üste Network hatası
/// olursa `network_down` bayrağı set edilir (indirmeler park edilir). Kısa
/// (tek seferlik) blip'lerde false-positive'i önler; 3 hata ~15-20s sürer.
const NETWORK_DOWN_THRESHOLD: u32 = 3;

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
        return (
            StatusCode::OK,
            [("Content-Type", "text/html; charset=utf-8")],
            html,
        )
            .into_response();
    }
    placeholder_index()
}

/// Placeholder index — static_root yoksa veya index.html okunamazsa.
fn placeholder_index() -> Response {
    let html = "<!doctype html><html><head><title>RGSX Manager</title></head>\
                <body><h1>RGSX Manager</h1></body></html>";
    (
        StatusCode::OK,
        [("Content-Type", "text/html; charset=utf-8")],
        html,
    )
        .into_response()
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
        .map(|t| {
            t.duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0)
                .to_string()
        })
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
    if safe.components().any(|c| {
        matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    }) {
        return (StatusCode::NOT_FOUND, "404 Not Found").into_response();
    }
    let file_path = root.join(&safe);
    match std::fs::read(&file_path) {
        Ok(bytes) => {
            let mime = mime_for(&file_path);
            (
                StatusCode::OK,
                [
                    ("Content-Type", mime),
                    ("Cache-Control", "public, max-age=3600"),
                ],
                bytes,
            )
                .into_response()
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

/// GET `/api/platforms` — Faz 10c/3/2: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn platforms(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/platforms").await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({ "count": 0, "platforms": [] })))
}

/// GET `/api/search?q=...` — Faz 10c/3/2: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    let term = params.get("q").cloned().unwrap_or_default();
    if let Some(c) = &state.catalog {
        let route = format!(
            "/api/search?q={}",
            percent_encoding::utf8_percent_encode(&term, percent_encoding::NON_ALPHANUMERIC)
        );
        if let Ok(v) = c.get_json(&route).await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({
        "search_term": term,
        "results": { "platforms": [], "games": [] },
    })))
}

/// GET `/api/translations` — Faz 10c/3/2: `catalog` varsa Python'a proxy, yoksa placeholder.
/// Opsiyonel `?lang=` ile dil seçilebilir (native backend).
pub async fn translations(
    Query(params): Query<HashMap<String, String>>,
    State(state): State<AppState>,
) -> Response {
    let lang = params.get("lang").cloned().unwrap_or_default();
    if let Some(c) = &state.catalog {
        let route = if lang.is_empty() {
            "/api/translations".to_string()
        } else {
            format!("/api/translations?lang={}", lang)
        };
        if let Ok(v) = c.get_json(&route).await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({
        "language": "tr",
        "translations": { "_language": "tr" },
    })))
}

/// GET `/api/languages` — TASK-003: mevcut dil kodlarını listeler (native backend).
pub async fn languages(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/languages").await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({ "languages": ["en", "tr"] })))
}

/// GET `/api/games/{platform}` — Faz 10c/3/2: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn games(
    State(state): State<AppState>,
    AxumPath(platform): AxumPath<String>,
) -> Response {
    if let Some(c) = &state.catalog {
        let route = format!(
            "/api/games/{}",
            percent_encoding::utf8_percent_encode(&platform, percent_encoding::NON_ALPHANUMERIC)
        );
        if let Ok(v) = c.get_json(&route).await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({
        "platform": platform,
        "count": 0,
        "games": [],
    })))
}

/// GET `/api/progress` — `config.download_progress` eşleniği.
/// Faz 10c/3/4: `catalog` varsa Python'a proxy (indirme durumu Python'da yaşar).
pub async fn progress(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/progress").await {
            return ok(v);
        }
    }
    let downloads = state.read().progress.clone();
    ok(contract::ok(json!({ "downloads": downloads })))
}

/// GET `/api/history` — history + message noise stripping.
/// Faz 10c/3/4: `catalog` varsa Python'a proxy (geçmiş Python'da yaşar).
pub async fn history(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/history").await {
            return ok(v);
        }
    }
    let history = state.read().history.clone();
    let cleaned: Vec<Value> = history
        .into_iter()
        .map(|mut entry| {
            if let Some(Value::String(msg)) = entry.get_mut("message") {
                *msg = contract::strip_history_error_noise(msg);
            }
            // TASK-012-gap-03 (bulgu 6): makine-okunur status_code enjeksiyonu.
            contract::with_status_code(entry)
        })
        .collect();
    ok(contract::ok(
        json!({ "count": cleaned.len(), "history": cleaned }),
    ))
}

/// GET `/api/queue` — kuyruk durumu.
/// Faz 10c/3/4: `catalog` varsa Python'a proxy (kuyruk Python'da yaşar).
pub async fn queue(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/queue").await {
            return ok(v);
        }
    }
    let data = state.read();
    // F3-F4: UI'ye O(1) "queued?" üyelik snapshot'ı ver (kilit mikrosaniyede bırakılır;
    // WebUI `Vec` taraması yapmaz, bu set ile doğrudan bakar).
    let queued_ids: Vec<String> = data.queued_ids.iter().cloned().collect();
    // TASK-012-gap-03 (bulgu 6): queue öğelerine makine-okunur status_code enjeksiyonu.
    let queue = contract::inject_status_codes(&data.queue);
    ok(contract::ok(json!({
        "active": data.active,
        "queue": queue,
        "queue_size": data.queue_size(),
        "queued_ids": queued_ids,
    })))
}

/// GET `/api/settings` — Faz 10c/3/3: `catalog` varsa Python'a proxy; `RGSX_NATIVE_SETTINGS=1`
/// ise native `Settings::load()` + `system_info`; yoksa placeholder.
pub async fn settings_get(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/settings").await {
            return ok(v);
        }
    }
    if manager_core::settings::native_enabled() {
        let s = manager_core::settings::Settings::load();
        return ok(contract::ok(json!({
            "settings": serde_json::to_value(&s).unwrap_or(json!({})),
            "system_info": manager_core::settings::system_info()
        })));
    }
    let settings = state.read().settings.clone();
    ok(contract::ok(json!({ "settings": settings })))
}

/// GET `/api/system_info` — Faz 10c/3/3: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn system_info(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.get_json("/api/system_info").await {
            return ok(v);
        }
    }
    let info = manager_core::settings::system_info();
    ok(contract::ok(json!({ "system_info": info })))
}

/// GET `/api/game-status` — Faz 12.6a: native katalog disk taramasını döndürür
/// (kurulu oyunlar `status:"downloaded"`), yoksa Python proxy'ye düşer.
pub async fn game_status(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        let native = c.game_statuses();
        let non_empty = native
            .get("statuses")
            .and_then(|s| s.as_object())
            .map(|m| !m.is_empty())
            .unwrap_or(false);
        if non_empty {
            return ok(contract::ok(native));
        }
        // Python proxy geriye uyum (NativeCatalog boş döndüyse de denenir).
        if let Ok(v) = c.get_json("/api/game-status").await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({ "statuses": {} })))
}

/// GET `/api/browse-directories?path=...` — Faz 10c/3/3: `catalog` varsa Python'a proxy, yoksa yerel placeholder.
pub async fn browse_directories(
    State(state): State<AppState>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Response {
    if let Some(c) = &state.catalog {
        let route = match params.get("path") {
            Some(p) => format!(
                "/api/browse-directories?path={}",
                percent_encoding::utf8_percent_encode(p, percent_encoding::NON_ALPHANUMERIC)
                    .to_string()
                    .replace("%2F", "/")
            ),
            None => "/api/browse-directories".to_string(),
        };
        if let Ok(v) = c.get_json(&route).await {
            return ok(v);
        }
    }
    let requested = params.get("path");
    if let Some(path) = requested {
        if !std::path::Path::new(path).exists() {
            return json_err("Le chemin spécifié n'existe pas", StatusCode::BAD_REQUEST);
        }
    }
    // Saf-Rust modda pathsiz çağrı kök yerine RGSX_DATA_DIR'i (yoksa cwd) listeler.
    let base = requested
        .cloned()
        .unwrap_or_else(|| match std::env::var("RGSX_DATA_DIR") {
            Ok(d) if std::path::Path::new(&d).is_dir() => d,
            _ => ".".to_string(),
        });
    let (current, dirs) = state.read().browse(&base);
    ok(contract::ok(
        json!({ "current_path": current, "directories": dirs }),
    ))
}

/// GET `/api/scan` — Faz 12d: `ROMS_FOLDER` (env `RGSX_ROMS_FOLDER`) HDD taraması.
/// `manager-scan` ile platform klasörlerine göre ROM dosyalarını toplar, disk
/// kullanımını ekler ve sonucu SSE `scan` olayı olarak yayar (canlı UI).
pub async fn scan(State(state): State<AppState>) -> Response {
    let root = std::env::var("RGSX_ROMS_FOLDER")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| ".".to_string());
    let path = std::path::Path::new(&root);
    let result = manager_scan::scan::scan_roms(path);
    let du = manager_scan::disk::disk_usage(path);
    let payload = serde_json::json!({
        "success": true,
        "root": result.root,
        "platforms": result.platforms,
        "total_bytes": result.total_bytes,
        "total_files": result.total_files,
        "disk": { "total": du.total, "used": du.used, "free": du.free },
    });
    crate::sse::publish(&state.events, "scan", &payload);
    ok(payload)
}

/// GET `/api/image/{platform}` — Faz 10c/3/2: `catalog` varsa Python'a proxy, yoksa 404 placeholder.
pub async fn image(
    State(state): State<AppState>,
    AxumPath(platform): AxumPath<String>,
) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok((bytes, ct)) = c.get_image(&platform).await {
            return (
                [
                    ("Content-Type", ct),
                    ("Access-Control-Allow-Origin", "*".to_string()),
                ],
                bytes,
            )
                .into_response();
        }
    }
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
    (
        StatusCode::NOT_FOUND,
        [
            ("Content-Type", "image/png"),
            ("Access-Control-Allow-Origin", "*"),
        ],
        PNG,
    )
        .into_response()
}

/// GET `/api/favicon` — 200 `image/x-icon` (placeholder).
pub async fn favicon() -> Response {
    (
        StatusCode::OK,
        [
            ("Content-Type", "image/x-icon"),
            ("Access-Control-Allow-Origin", "*"),
        ],
        b"\x00\x00\x01\x00".as_slice(),
    )
        .into_response()
}

/// GET `/api/update-cache` — placeholder `deleted: 0`.
pub async fn update_cache() -> Response {
    ok(contract::ok(json!({ "deleted": 0 })))
}

// ---------------------------------------------------------------------------
// TASK-012m — manager self-update (Faz 1-4 güvenli iskelet; Faz 5 replace yok)
// ---------------------------------------------------------------------------

/// GET `/api/manager-update` — bekleyen self-update durumunu döndürür
/// (`{available,version,url,sha256}` ya da `null`).
pub async fn manager_update_status(State(state): State<AppState>) -> Response {
    let info = state.data.read().unwrap().manager_update.clone();
    ok(contract::ok(json!({ "update": info })))
}

/// POST `/api/manager-update/download` — Faz 5: self-update binary'sini arka plan
/// görevi olarak indirir (kuyruk + iptal edilebilir, SHA256 doğrulamalı). Non-blocking:
/// hemen `{ok:true, queued:true}` döner; ilerleme SSE `manager_update`/`progress` ile gelir.
pub async fn manager_update_download(State(state): State<AppState>) -> Response {
    let pending = state.data.read().unwrap().manager_update.clone();
    let Some(info) = pending else {
        return ok(contract::ok(
            json!({ "ok": false, "error": "bekleyen güncelleme yok" }),
        ));
    };
    let stage = info
        .get("stage")
        .and_then(|v| v.as_str())
        .unwrap_or("available");
    if stage != "available" {
        return ok(contract::ok(
            json!({ "ok": false, "error": format!("zaten {stage} aşamasında") }),
        ));
    }
    let url = match info.get("url").and_then(|v| v.as_str()) {
        Some(u) if !u.is_empty() => u.to_string(),
        _ => return ok(contract::ok(json!({ "ok": false, "error": "url yok" }))),
    };
    let sha = info
        .get("sha256")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let events = state.events.clone();
    crate::self_update::start_update_download(state.clone(), events, url, sha);
    ok(contract::ok(json!({ "ok": true, "queued": true })))
}

/// POST `/api/manager-update/apply` — Faz 5: indirilen binary'yi uygula (replace +
/// relaunch). Serviste reddedilir; gerçek replace yalnız `RGSX_SELF_APPLY=1` ile
/// (kullanıcının açık "evet"i). GERİ ALINAMAZ adım.
pub async fn manager_update_apply(State(state): State<AppState>) -> Response {
    let events = state.events.clone();
    match crate::self_update::apply_update(state.clone(), events).await {
        Ok(()) => ok(contract::ok(json!({ "ok": true }))),
        Err(e) => ok(contract::ok(json!({ "ok": false, "error": e }))),
    }
}

/// POST `/api/catalog/retry` — TASK-012h bootstrap-fail UX: katalog hazırlanması
/// başarısız olduysa (no_source / download_failed / extract_failed) yeniden dener.
/// `ensure_catalog_ready` arka planda tetiklenir; ilerleme yine SSE `catalog_update`
/// olaylarıyla TVUI/WebUI'ye ulaşır. Hemen `{retrying:true}` döner.
pub async fn catalog_retry(State(state): State<AppState>) -> Response {
    let events = state.events.clone();
    let data = state.data.clone();
    tokio::spawn(async move {
        crate::catalog_bootstrap::ensure_catalog_ready(Some(&events), Some(data)).await;
    });
    ok(contract::ok(json!({ "retrying": true })))
}

// ---------------------------------------------------------------------------
// POST — web
// ---------------------------------------------------------------------------

/// POST `/api/download` — doğrulama sırası Python `_handle_download_worker`
/// (rgsx_manager.py:337) ile birebir; başarı `Queued` + arka plan indirme.
///
/// Bridge (librqbit engine) varsa url+game_name gerçekten indirmeye başlar:
/// `downloads_folder` + türetilmiş dosya adıyla `download_torrent` arka plan
/// task'ında koşar, bitince history/downloaded/progress + SSE sonuçlanır.
/// Bridge yoksa (pure placeholder) eski davranış korunur — yalnız kuyruklanır.
///
/// TASK-002-gap-12 (dedup): Aynı URL için eşzamanlı çift spawn'ı engeller.
///
/// `retry_in_flight` set'ine atomik check-and-insert yapar; URL zaten aktifse
/// `false` döner (caller yinelenen isteği düşürür). Python `queue.py:649-685`
/// `urls_in_progress` dedup parity'si — çift indirme → partial/corrupt çakışması
/// olmadan mevcut görev sonlanır.
fn claim_in_flight(state: &AppState, url: &str) -> bool {
    let mut d = state.write();
    if d.retry_in_flight.contains(url) {
        return false;
    }
    d.retry_in_flight.insert(url.to_string());
    true
}

/// Not: tüm `.await`'ler `state.write()` kilidinden ÖNCE — write guard sonrası
/// await handler future'ını Send yapmaz (bkz. change_password deseni).
pub async fn download(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    // TASK-002l: doğrudan çözülmüş torrent URL'i (magnet:/rgsx+torrent:/.torrent) ve
    // bir bridge (librqbit varsayılan) mevcutsa, indirme engine'e yönlendirilir
    // (canlıda catalog olsa bile).
    let direct_url = body.get("url").and_then(Value::as_str);
    let intercept_locally =
        direct_url.map(is_torrent_url).unwrap_or(false) && state.bridge.is_some();

    // Faz 12e: native DDL çözümü + doğrudan HTTP indirme. Debrid yapılandırılmamışsa
    // `DownloadManager` DirectResolver'a düşer ve düz HTTP kaynak doğrudan indirilir.
    // gap-27: saf-Rust varsayılan = true (native DDL açık). Flag yine env ile override edilebilir
    // (contract test izolasyonu). `native_download` her iki native DDL kesiğinde de kullanılır.
    let native_download = std::env::var("RGSX_NATIVE_DOWNLOAD")
        .map(|v| v == "1")
        .unwrap_or(true);
    if native_download {
        if let Some(direct) = direct_url {
            if !is_torrent_url(direct) {
                // 1fichier provider zinciri (TASK-002-gap-11) — debrid/free fallback.
                if is_onefichier_url(direct) {
                    let platform = body
                        .get("platform")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let game_name = body
                        .get("game_name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if platform.is_empty() || game_name.is_empty() {
                        return json_err(
                            "Paramètre manquant: platform et game_name requis (1fichier)",
                            StatusCode::BAD_REQUEST,
                        );
                    }
                    return native_onefichier_download(
                        state,
                        direct.to_string(),
                        platform,
                        game_name,
                    )
                    .await;
                }
                if let Ok(manager_download::DownloadSource::DirectHttp(resolved)) =
                    manager_download::DownloadManager::new().resolve(direct)
                {
                    let platform = body
                        .get("platform")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let game_name = body
                        .get("game_name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    if platform.is_empty() || game_name.is_empty() {
                        return json_err(
                            "Paramètre manquant: platform et game_name requis (native DDL)",
                            StatusCode::BAD_REQUEST,
                        );
                    }
                    return native_ddl_download(
                        state,
                        direct.to_string(),
                        resolved,
                        platform,
                        game_name,
                    )
                    .await;
                }
            }
        }
    }

    if !intercept_locally {
        // Katalog kaynaklı POST delegasyonu (NativeCatalog post_json hep Err döner;
        // blok test sahteleri/FakeCatalog için genel sözleşme olarak kalır).
        if let Some(c) = &state.catalog {
            if let Ok(v) = c.post_json("/api/download", &body).await {
                return ok(v);
            }
        }
    }
    let platform = body.get("platform").and_then(Value::as_str);
    let game_index = body.get("game_index");
    let game_name = body.get("game_name").and_then(Value::as_str);

    if platform.is_none() || (game_index.is_none() && game_name.is_none() && direct_url.is_none()) {
        return json_err(
            "Paramètres manquants: platform et (game_index ou game_name) requis",
            StatusCode::BAD_REQUEST,
        );
    }

    if let Some(direct) = direct_url {
        let Some(gname) = game_name else {
            return json_err(
                "Paramètre manquant: game_name requis avec url",
                StatusCode::BAD_REQUEST,
            );
        };
        let platform = platform.unwrap().to_string();
        let game_url = direct.to_string();
        let gname = gname.to_string();
        let task_id = web_task_id();

        // DÜZELTME: archive.org gibi düz HTTP (.zip) kaynakları torrent engine'ine
        // (librqbit) GÖNDERİLMEMELİ — aksi halde "error decoding torrent" hatası verir.
        // Katalog yoksa (native mod) doğrudan HTTP indirme yapılır. Native DDL kapalıyken
        // bu blok atlanır ve istek bridge/placeholder yoluna düşer (flag parity + test izolasyonu).
        if !is_torrent_url(&game_url) && native_download {
            if is_onefichier_url(&game_url) {
                return native_onefichier_download(state, game_url, platform, gname).await;
            }
            match manager_download::DownloadManager::new().resolve(&game_url) {
                Ok(manager_download::DownloadSource::DirectHttp(resolved)) => {
                    return native_ddl_download(state, game_url, resolved, platform, gname).await;
                }
                _ => {
                    return json_err(
                        "HTTP kaynağı çözümlenemedi (direct download desteklenmiyor)",
                        StatusCode::BAD_REQUEST,
                    );
                }
            }
        }

        // Bridge varsa indirmeyi başlat (arka plan task; yanıt beklemez).
        // Await'ler kilit öncesi — spawn closure'u `'static' olduğundan değerler klonlanır.
        if let Some(bridge) = state.bridge.clone() {
            let downloads = bridge
                .get_app_paths()
                .await
                .map(|(d, _)| d)
                .unwrap_or_default();
            // Faz 10c/2: Python, kendi hedef yolunu (`dest_path`) verebilir; yoksa
            // eski davranış — `downloads_folder` + türetilen dosya adı (geriye uyumlu).
            let dest_path = match body.get("dest_path").and_then(Value::as_str) {
                Some(p) if !p.is_empty() => std::path::PathBuf::from(p),
                _ => {
                    let roms_dest = match effective_roms_folder() {
                        Some(rf) => rom_dest_for(&rf, &platform, &gname, &game_url),
                        None => dest_path_for(&downloads, &game_url, &gname),
                    };
                    // gap-28: BIOS benzeri platformlar USERDATA_FOLDER'a yönlenir.
                    redirect_bios_dest(roms_dest, &platform, &gname)
                }
            };
            let state2 = state.clone();
            let u = game_url.clone();
            let n = gname.clone();
            let p = platform.clone();
            let t = task_id.clone();
            // TASK-002-gap-12 (dedup): aynı URL zaten aktifse yinelenen spawn'ı
            // düşür. TASK-002-gap-1: retry döngüsü (torrent + native DDL ortak
            // envelope) — her transient başarısızlıkta yeni task_id + yeni history
            // entry (Python queue.py:610 parity). cancel/shutdown sinyalleri aşağıda.
            if !claim_in_flight(&state, &game_url) {
                return ok(contract::ok(json!({
                    "queued": false,
                    "message": "Déjà en cours de téléchargement",
                    "url": game_url,
                    "task_id": Value::Null,
                })));
            }
            let cancel: Arc<Notify> = {
                let mut d = state.write();
                let sig = Arc::new(Notify::new());
                d.cancel_signals.insert(game_url.clone(), sig.clone());
                sig
            };
            let shutdown = state.shutdown.clone();
            tokio::spawn(async move {
                // Faz 12.6d: eşzamanlı indirme sınırı (max_simultaneous_downloads).
                // Semaphore izni task sonuna kadar tutulur → concurrency kontrolü.
                // `queue_clear` (stop-all) semaphore'u kapatınca acquire Err → görev çıkar.
                let _dl_permit = {
                    let sem = state2.read().download_semaphore.clone();
                    match sem.acquire_owned().await {
                        Ok(p) => p,
                        Err(_) => {
                            let mut d = state2.write();
                            d.retry_in_flight.remove(&u);
                            d.cancel_signals.remove(&u);
                            d.pause_signals.remove(&u);
                            return;
                        }
                    }
                };
                if state2.read().aborting.load(Ordering::SeqCst) {
                    let mut d = state2.write();
                    d.retry_in_flight.remove(&u);
                    d.cancel_signals.remove(&u);
                    d.pause_signals.remove(&u);
                    return;
                }
                let mut current_task_id = t.clone();
                let current_url = u.clone();
                let mut aborted: Option<String> = None;
                loop {
                    // TASK-002-gap-32: ağ koptuysa indirmeyi başlatmadan PARK et
                    // (retry budget yakma). Bağlantı dönünce reconnect-probe ile uyan.
                    let down = state2.read().network_down.load(Ordering::Relaxed);
                    if down {
                        if probe_connectivity().await {
                            let mut d = state2.write();
                            d.network_down.store(false, Ordering::SeqCst);
                            d.network_error_streak.store(0, Ordering::SeqCst);
                            for q in d.queue.iter_mut() {
                                if q.get("status").and_then(|v| v.as_str()) == Some("Ağ bekleniyor")
                                {
                                    q["status"] = json!("Downloading");
                                }
                            }
                            d.network_resume.notify_waiters();
                            state2.dirty.store(true, Ordering::SeqCst);
                            // TASK-002-gap-32: yalnızca GERÇEK kesinti sonrası restore bildirimi
                            // (ölü-host titreşiminde spam olmasın). Onay bayrağı tek emitçi garantiler.
                            if d.network_outage_confirmed
                                .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                                .is_ok()
                            {
                                crate::sse::publish(
                                    &state2.events,
                                    "network_restored",
                                    &serde_json::json!({ "network_down": false }),
                                );
                            }
                        } else {
                            state2
                                .write()
                                .network_outage_confirmed
                                .store(true, Ordering::SeqCst);
                            let nr = state2.read().network_resume.clone();
                            let _ =
                                tokio::time::timeout(Duration::from_millis(1000), nr.notified())
                                    .await;
                            continue;
                        }
                    }
                    let cb_state = state2.clone();
                    let cb_url = current_url.clone();
                    // F6: progress SSE'i artık `broadcast_loop` (250ms batched delta)
                    // tarafından yayınlanır; burada yalnızca durum yazılır.
                    let on_progress: Option<
                        Arc<dyn Fn(manager_bridge::ProgressEvent) + Send + Sync>,
                    > = Some(Arc::new(move |ev: manager_bridge::ProgressEvent| {
                        let pct = if ev.total > 0 {
                            ((ev.downloaded as f64 / ev.total as f64) * 100.0) as u64
                        } else {
                            0
                        };
                        let mut data = cb_state.write();
                        if let Value::Object(map) = &mut data.progress {
                            map.insert(
                                cb_url.clone(),
                                json!({
                                    "status": if ev.finished {
                                        "Download_OK"
                                    } else if ev.paused {
                                        "Paused"
                                    } else {
                                        "Downloading"
                                    },
                                    "progress": pct,
                                    "downloaded": ev.downloaded,
                                    "total": ev.total,
                                    "speed": ev.speed,
                                }),
                            );
                        }
                    }));
                    // GAP-6: indirme sonrası otomatik çıkarma ipucu üret.
                    // `platform` + `get_auto_extract()` + URL uzantısından türetilir
                    // (Python `should_force_extract` parity: BIOS/PS3 zorunlu, ya da
                    // zip/rar + auto_extract). Catalog yoksa (native) URL uzantısı
                    // `is_zip_non_supported` için makul bir heuristik'tir.
                    let auto_extract = manager_core::settings::Settings::load().get_auto_extract();
                    let is_zip = std::path::Path::new(&current_url)
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.eq_ignore_ascii_case("zip") || e.eq_ignore_ascii_case("rar"))
                        .unwrap_or(false);
                    let extract_hint = manager_core::extract::ExtractHint {
                        auto_extract,
                        is_zip_non_supported: is_zip,
                        platform_folder: p.to_ascii_lowercase(),
                        platform: p.clone(),
                    };
                    let res = bridge
                        .download_torrent_progress(
                            &current_url,
                            &dest_path,
                            Some(current_task_id.clone()),
                            on_progress,
                            Some(extract_hint),
                        )
                        .await;
                    match res {
                        Ok(src) => {
                            state2
                                .write()
                                .network_error_streak
                                .store(0, Ordering::SeqCst);
                            finalize_download_in_state(
                                &state2,
                                &current_task_id,
                                &current_url,
                                &n,
                                &p,
                                true,
                                src.to_string_lossy().as_ref(),
                            )
                            .await;
                            break;
                        }
                        Err(e) => {
                            let cls = classify_bridge_error(&e);
                            let is_network = matches!(e, BridgeError::Timeout(_));
                            match decide_retry(
                                &state2,
                                &current_url,
                                &n,
                                &current_task_id,
                                &e.to_string(),
                                cls,
                                is_network,
                            )
                            .await
                            {
                                RetryDecision::Retry { new_task_id, delay } => {
                                    let dur = Duration::from_secs_f64(delay.max(0.0));
                                    tokio::select! {
                                        _ = tokio::time::sleep(dur) => {}
                                        _ = cancel.notified() => { aborted = Some("İptal edildi".to_string()); }
                                        _ = shutdown.notified() => { aborted = Some("Sunucu kapatılıyor".to_string()); }
                                    }
                                    match aborted {
                                        Some(ref msg) => {
                                            finalize_download_in_state(
                                                &state2,
                                                &current_task_id,
                                                &current_url,
                                                &n,
                                                &p,
                                                false,
                                                msg,
                                            )
                                            .await;
                                            break;
                                        }
                                        None => {
                                            current_task_id = new_task_id;
                                            continue;
                                        }
                                    }
                                }
                                RetryDecision::Stop => {
                                    finalize_download_in_state(
                                        &state2,
                                        &current_task_id,
                                        &current_url,
                                        &n,
                                        &p,
                                        false,
                                        &e.to_string(),
                                    )
                                    .await;
                                    break;
                                }
                            }
                        }
                    }
                }
                let mut d = state2.write();
                d.retry_in_flight.remove(&current_url);
                d.cancel_signals.remove(&current_url);
            });
        }

        push_queued_history_entry(
            &state,
            &task_id,
            &game_url,
            &gname,
            &platform,
            "Queued",
            "Ajouté à la file d'attente",
            0,
        );
        let queue_position = state.read().queue.len();

        return ok(contract::ok(json!({
            "queued": true,
            "game_name": gname,
            "platform": platform,
            "task_id": task_id,
            "message": format!("{gname} ajouté à la file d'attente"),
            "queue_position": queue_position,
        })));
    }

    // Katalog placeholder boş → game_index/game_name asla bulunamaz (TASK-002c).
    if let Some(name) = game_name {
        return json_err(format!("Jeu non trouvé: {name}"), StatusCode::BAD_REQUEST);
    }
    let idx = game_index.and_then(Value::as_i64).unwrap_or(-1);
    json_err(
        format!("Index de jeu invalide: {idx}"),
        StatusCode::BAD_REQUEST,
    )
}

/// POST `/api/download/batch` — Faz 12.6d: native modda `platform + game_names`
/// listesini katalogdan çözüp tek tek kuyruğa ekler; eşzamanlı indirme `download_semaphore`
/// ile sınırlıdır (ayar: `max_simultaneous_downloads`). `catalog` varsa (Python) eski proxy
/// davranışı korunur.
pub async fn download_batch(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/download/batch", &body).await {
            return ok(v);
        }
    }
    let platform = body
        .get("platform")
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let Some(names) = body.get("game_names").and_then(|v| v.as_array()) else {
        return json_err("Paramètre 'game_names' manquant", StatusCode::BAD_REQUEST);
    };
    if platform.is_empty() {
        return json_err("Paramètre 'platform' manquant", StatusCode::BAD_REQUEST);
    }

    // Yeni batch = kullanıcı indirme niyeti → önceki "stop all" bayrağını temizle
    // (taze indirmeler hemen başlasın). `pending_set` temizliği consumer'ı
    // durdurur; burada sadece genel aborting'i sıfırlarız.
    state.write().aborting.store(false, Ordering::SeqCst);

    let mut queued = 0u32;
    let mut skipped = 0u32;
    let mut already_downloaded = 0u32;
    let mut errors: Vec<String> = Vec::new();
    let mut results: Vec<Value> = Vec::new();

    // F1 (gap-30): Handler YALNIZCA katalog çözümü + dedupe + sayım yapar ve
    // geçerli öğeleri tek bir `QueueCommand::AddBatch` mesajıyla worker'a yollar
    // (O(1) dönüş — binlerce oyun handler'ı bloke etmez). Worker SÜREKLİ çalışır;
    // gerçek indirme spawn'ı arka plan `queue_worker` döngüsünde yapılır. "Durdur"
    // (`queue_clear`) `pending_set`'i temizler → taze büyüme durur.
    // F3-F4: dedupe O(1) — `retry_in_flight` (aktif) + `queued_ids` (buffer'da) +
    // `downloaded_index` (kurulu) hepsi HashSet, kilit altında mikrosaniye clone.
    let active: HashSet<String> = state.read().retry_in_flight.clone();
    let buffered: HashSet<String> = state.read().queued_ids.clone();
    let downloaded_index: HashSet<(String, String)> = state.read().downloaded_index.clone();
    let mut items: Vec<QueuedItem> = Vec::with_capacity(names.len());
    for n in names {
        let name = n.as_str().unwrap_or("").to_string();
        if name.is_empty() {
            skipped += 1;
            continue;
        }
        let url = state
            .catalog
            .as_ref()
            .and_then(|c| c.game_url(&platform, &name));
        let Some(url) = url else {
            skipped += 1;
            errors.push(format!("Jeu introuvable: {name}"));
            results.push(json!({ "ok": false, "game_name": name, "error": "not_found" }));
            continue;
        };
        if active.contains(&url) || buffered.contains(&url) {
            skipped += 1;
            results.push(json!({ "ok": false, "game_name": name, "error": "already_queued" }));
            continue;
        }
        if downloaded_index.contains(&(platform.clone(), name.clone())) {
            already_downloaded += 1;
            skipped += 1;
            results.push(json!({ "ok": false, "game_name": name, "error": "already_downloaded" }));
            continue;
        }
        items.push(QueuedItem {
            platform: platform.clone(),
            name: name.clone(),
            url,
        });
        queued += 1;
        results.push(json!({ "ok": true, "game_name": name }));
    }

    // Worker'a tek mesajla yolla (kanal bounded; tek mesaj → asla dolmaz/takılmaz).
    if !items.is_empty() {
        let _ = state.tx.send(QueueCommand::AddBatch(items)).await;
    }

    ok(contract::ok(json!({
        "queued": queued,
        "skipped": skipped,
        "already_downloaded": already_downloaded,
        "total": names.len(),
        "errors": errors,
        "results": results,
    })))
}

/// Arka plan kuyruk worker'ı (Faz gap-30 / F1) — `AppState::empty()`/`with_data()`
/// ve `main.rs` tarafından `tokio::spawn` ile başlatılır, SÜREKLİ çalışır.
///
/// Çıkış noktası TEK: `rx.recv()` `None` döndüğünde (tüm `tx` drop edildiğinde,
/// yani uygulama kapanışında). `Paused`/`Stopped` durumunda `break`/`return` YOK.
/// Gelen komutlar `pending_set`'e yazılır (mesaj ASLA drop edilmez) ve buffer
/// boşalana kadar dispatch edilir. F2'de dispatch `status == Running` kapısıyla
/// gate'lenecek; `Paused→Running` geçişi `pending_notify` ile uyandıracak.
pub async fn queue_worker(mut rx: mpsc::Receiver<QueueCommand>, state: AppState) {
    loop {
        // 1) Dispatch YALNIZCA `status == Running` iken. Paused'ta `pop_front`
        //    ENGELLENİR; gelen Add/AddBatch öğeleri `pending_set`'te KALIR (drop
        //    EDİLMEZ) → resume'da baştan işlenir. Kilit yalnızca `pop_front`
        //    süresince tutulur; `download` çağrısı kilit DIŞINDA → deadlock yok.
        while state.read().status == QueueStatus::Running {
            let url = {
                let mut d = state.write();
                d.pending_set.pop_front()
            };
            match url {
                Some(url) => {
                    // Dispatch anında: buffer üyeliğini düş, durumu Active yap,
                    // payload'ı queued_items'tan O(1) al. `queued_ids` artık
                    // aktif olduğu için "queued?" snapshot'ından çıkar.
                    let item = {
                        let mut d = state.write();
                        d.queued_ids.remove(&url);
                        d.tasks.insert(url.clone(), TaskState::Active);
                        d.queued_items.remove(&url)
                    };
                    state
                        .dirty
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    if let Some(item) = item {
                        let _ = dispatch_queued(&state, item).await;
                    }
                    continue;
                }
                None => break,
            }
        }
        // 2) Buffer boş VEYA Paused → yeni komut VEYA resume sinyali beklenir
        //    (ikisi de döngüyü döndürür; resume'da yukarıdaki `while` drene eder).
        let resume = state.read().pending_notify.clone();
        tokio::select! {
            cmd = rx.recv() => match cmd {
                Some(QueueCommand::Add(item)) => {
                    let mut d = state.write();
                    let url = item.url.clone();
                    d.pending_set.push_back(url.clone());
                    d.queued_items.insert(url.clone(), item);
                    d.tasks.insert(url.clone(), TaskState::Queued);
                    d.queued_ids.insert(url);
                    state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                Some(QueueCommand::AddBatch(items)) => {
                    let mut d = state.write();
                    for item in items {
                        let url = item.url.clone();
                        d.pending_set.push_back(url.clone());
                        d.queued_items.insert(url.clone(), item);
                        d.tasks.insert(url.clone(), TaskState::Queued);
                        d.queued_ids.insert(url);
                        state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                // Tüm sender'lar drop → worker biter (meşru shutdown).
                None => return,
            },
            // Paused→Running geçişinde `pending_notify.notify_waiters()` ile çalar.
            _ = resume.notified() => {}
            // gap-30: missed-wakeup koruması. `notify_waiters()` bekleyen yokken
            // çağrılırsa notify KAYBOLUR → worker sonsuza dek park kalır (status
            // Running olmasına rağmen indirme durur, "devam etmiyor"). Periyodik
            // timeout ile döngü tepesine dönülür; `while status==Running` yeniden
            // dispatch eder. Paused iken 1sn'de bir uyanıp yeniden park (önemsiz).
            _ = tokio::time::sleep(Duration::from_millis(1000)) => {}
        }
    }
}

/// Tek bir `QueuedItem`'ı `download()` handler'ına yönlendirir. `download` kendi
/// içinde arka plan task spawn eder ve hızlı döner; worker yalnız kurulumu bekler.
async fn dispatch_queued(state: &AppState, item: QueuedItem) {
    eprintln!("[TRACE] dispatch url={}", item.url);
    let single = json!({
        "url": item.url,
        "platform": item.platform,
        "game_name": item.name,
        "mode": "queue",
    });
    let _ = download(State(state.clone()), Json(single)).await;
}

/// POST `/api/cancel` — Faz 10c/3/4 + Gap-3: `catalog` varsa Python'a proxy;
/// yoksa bridge'e `cancel_torrent` (task_id) / `cancel_all` (task_id yoksa).
/// Bridge yoksa placeholder (geriye uyum).
pub async fn cancel(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/cancel", &body).await {
            return ok(v);
        }
    }
    let url = body.get("url").and_then(Value::as_str);
    let task_id = body.get("task_id").and_then(Value::as_str);
    // TASK-002-gap-1: native DDL retry döngüsünü uyandır (retry motoru cancel).
    if let Some(u) = url {
        if let Some(sig) = state.write().cancel_signals.get(u).cloned() {
            sig.notify_one();
        }
    }
    if let Some(bridge) = &state.bridge {
        let canceled = match task_id {
            Some(id) => bridge.cancel_torrent(id).await.unwrap_or(false),
            None => bridge.cancel_all().await.unwrap_or(0) > 0,
        };
        return ok(contract::ok(json!({
            "message": "Téléchargement annulé",
            "url": url.unwrap_or_default(),
            "task_id": task_id.unwrap_or_default(),
            "canceled": canceled,
        })));
    }
    let Some(url) = url else {
        return json_err("Paramètre manquant: url requis", StatusCode::BAD_REQUEST);
    };
    ok(contract::ok(json!({
        "message": "Téléchargement annulé",
        "url": url,
        "task_id": Value::Null,
    })))
}

/// POST `/api/queue` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa `queue_size`.
pub async fn queue_post(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/queue", &Value::Null).await {
            return ok(v);
        }
    }
    let size = state.read().queue_size();
    ok(contract::ok(json!({ "queue_size": size })))
}

/// POST `/api/queue/clear` — Tüm kuyruğu temizler VE devam eden/bekleyen tüm
/// indirmeleri iptal eder (gerçek "stop all"). `catalog` varsa Python'a proxy.
///
/// Mekanizma:
/// 1. `download_semaphore.close()` → semaphore'da bekleyen (queued) görevler
///    `acquire` Err alır ve çıkar (native_ddl_download / bridge yolu).
/// 2. Yeni semaphore aynı kapasiteyle yeniden oluşturulur (gelecek indirmeler için).
/// 3. Tüm `cancel_signals` + `pause_signals` tetiklenir → aktif indirmeler abort olur.
/// 4. `aborting` bayrağı set edilir (geç klonlanan görevleri yakalamak için) ve
///    kısa gecikmeyle (600ms) sıfırlanır.
/// 5. Kuyruk, `pending_set` (arka plan batch tüketicisi), retry_in_flight,
///    sinyal haritaları ve progress temizlenir. Consumer uyanır, boş seti bulup
///    bekler → "Download All" listesi yeniden büyümez.
pub async fn queue_clear(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/queue/clear", &Value::Null).await {
            return ok(v);
        }
    }
    let cleared = {
        let mut data = state.write();
        // 1+2) Bekleyen görevleri uyandır, yeni semaphore kur.
        data.download_semaphore.close();
        let cap = data.max_simultaneous_downloads;
        data.download_semaphore = Arc::new(tokio::sync::Semaphore::new(cap));
        // 3) Aktif indirmeleri iptal et.
        for sig in data.cancel_signals.values() {
            sig.notify_one();
        }
        for sig in data.pause_signals.values() {
            sig.notify_one();
        }
        // 4) Geç klonlanan görevleri yakala.
        data.aborting.store(true, Ordering::SeqCst);
        // 5) Durum temizliği.
        let cleared = data.queue.len();
        data.queue.clear();
        // 5b) Arka plan batch worker'ının buffer'ını + O(1) indekslerini temizle →
        // "Download All" listesi durur, yeniden büyümez. `downloaded_index` KORUNUR
        // (kurulu oyunlar hâlâ indirilmiş sayılır).
        data.pending_set.clear();
        data.queued_items.clear();
        data.queued_ids.clear();
        data.clear_tasks();
        data.retry_in_flight.clear();
        data.cancel_signals.clear();
        data.pause_signals.clear();
        data.active = false;
        data.progress = json!({});
        // SSE: kuyruk/aktif/progress sıfırlandı → yayıncıyı uyandır (idle'da noop olur).
        state
            .dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        // 5c) Consumer'ı uyandır (boş seti bulup beklemeye geçer).
        data.pending_notify.notify_one();
        // F6: queue/progress SSE'i `broadcast_loop` (250ms) yayınlar.
        cleared
    };
    // aborting bayrağını kısa gecikmeyle sıfırla: bu pencerede klonlanan eski
    // görevler iptal olsun, yeni kullanıcı indirmeleri etkilenmesin.
    let st = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        st.write().aborting.store(false, Ordering::SeqCst);
    });
    ok(contract::ok(json!({
        "cleared_count": cleared,
        "message": format!("{cleared} éléments supprimés — tous les téléchargements annulés"),
    })))
}

/// POST `/api/queue/remove` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa yerel.
pub async fn queue_remove(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/queue/remove", &body).await {
            return ok(v);
        }
    }
    let Some(task_id) = body.get("task_id").and_then(Value::as_str) else {
        return json_err(
            "Paramètre manquant: task_id requis",
            StatusCode::BAD_REQUEST,
        );
    };
    let task_id = task_id.to_string();
    // TASK-012m Faz 5: self-update indirmesi iptali (WebUI/Python TVUI parity).
    if task_id == crate::self_update::MANAGER_UPDATE_TASK_ID {
        crate::self_update::cancel_update_download(&state);
    }
    let mut data = state.write();
    if let Some(pos) = data
        .queue
        .iter()
        .position(|e| e.get("task_id").and_then(Value::as_str) == Some(task_id.as_str()))
    {
        data.queue.remove(pos);
        if let Some(obj) = data.progress.as_object_mut() {
            obj.remove(task_id.as_str());
        }
        data.active = !data.queue.is_empty();
        state
            .dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        // F6: queue SSE'i `broadcast_loop` (250ms) yayınlar.
        return ok(contract::ok(json!({ "task_id": task_id })));
    }
    json_err(
        format!("Élément non trouvé: {task_id}"),
        StatusCode::NOT_FOUND,
    )
}

/// POST `/api/settings` — Faz 10c/3/3: `catalog` varsa Python'a proxy; `RGSX_NATIVE_SETTINGS=1`
/// ise native validasyon + `Settings::save()`; yoksa placeholder (in-memory).
pub async fn settings_post(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/settings", &body).await {
            return ok(v);
        }
    }
    let Some(settings) = body.get("settings") else {
        return json_err("Paramètre \"settings\" manquant", StatusCode::BAD_REQUEST);
    };
    if manager_core::settings::native_enabled() {
        match serde_json::from_value::<manager_core::settings::Settings>(settings.clone()) {
            Ok(s) => {
                // TASK-012-gap-03: kayıt yolunda da coercion (ör. geçersiz grid → 3x4);
                // normalize yalnız load()'da koşuyordu, POST ile bayat değer yazılabiliyordu.
                let s = s.normalized();
                match s.validate() {
                    Ok(()) => match s.save() {
                        Ok(()) => {
                            // Faz 12.6d: eşzamanlı indirme sınırını güncelle.
                            let cap = (s.max_simultaneous_downloads.max(1)) as usize;
                            {
                                let mut data = state.write();
                                data.download_semaphore =
                                    Arc::new(tokio::sync::Semaphore::new(cap));
                                data.max_simultaneous_downloads = cap;
                            }
                            // Faz 12.6e: `roms_folder` değişmiş olabilir → kurulu oyun
                            // snapshot'ını (İndirilenler sekmesi + yeşil rozetler) tazele.
                            if let Some(c) = &state.catalog {
                                let installed = c.installed_list();
                                let mut d = state.write();
                                d.downloaded = serde_json::json!(installed);
                                // F3-F4: O(1) indeksi tazele (batch dedupe bunu kullanır).
                                d.rebuild_downloaded_index();
                                state
                                    .dirty
                                    .store(true, std::sync::atomic::Ordering::Relaxed);
                            }
                            return ok(contract::ok(Value::Null));
                        }
                        Err(e) => {
                            return json_err(
                                format!("Sauvegarde des paramètres échouée: {e}"),
                                StatusCode::INTERNAL_SERVER_ERROR,
                            )
                        }
                    },
                    Err(e) => {
                        return json_err(
                            format!("Paramètres invalides: {e}"),
                            StatusCode::BAD_REQUEST,
                        )
                    }
                }
            }
            Err(e) => {
                return json_err(
                    format!("Paramètres invalides: {e}"),
                    StatusCode::BAD_REQUEST,
                )
            }
        }
    }
    state.write().settings = settings.clone();
    ok(contract::ok(Value::Null))
}

/// POST `/api/save_filters` — Faz 10c/3/3: `catalog` varsa Python'a proxy; native modda
/// gövdedeki filtre alanlarını `Settings.game_filters` (`extra`) içine kalıcılaştırır.
///
/// Frontend `saveFilters()` `{region_filters, hide_non_release, one_rom_per_game,
/// hide_downloaded, regex_mode, region_priority}` gönderir; eski Python
/// `_api_save_filters` parity'siyle `rgsx_settings.json`'a yazar.
pub async fn save_filters(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/save_filters", &body).await {
            return ok(v);
        }
    }
    if manager_core::settings::native_enabled() {
        let mut s = manager_core::settings::Settings::load();
        let mut game_filters = serde_json::Map::new();
        for key in [
            "region_filters",
            "hide_non_release",
            "one_rom_per_game",
            "hide_downloaded",
            "regex_mode",
            "region_priority",
        ] {
            if let Some(v) = body.get(key) {
                game_filters.insert(key.to_string(), v.clone());
            }
        }
        s.extra
            .insert("game_filters".to_string(), Value::Object(game_filters));
        if let Err(e) = s.save() {
            return json_err(
                format!("Filtre kaydetme başarısız: {e}"),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
        return ok(contract::ok(json!({ "message": "Filtres sauvegardés" })));
    }
    ok(contract::ok(json!({ "message": "Filtres sauvegardés" })))
}

/// POST `/api/clear-history` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa yerel.
pub async fn clear_history(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/clear-history", &Value::Null).await {
            return ok(v);
        }
    }
    // TASK-002-gap-10 (B): yalnızca aktif OLMAYAN entry'leri sil — Python
    // `clear_history` `is_truly_active` parity'si (aktif indirme korunur).
    let (preserved, path) = {
        let mut data = state.write();
        let queue_ids: HashSet<String> = data
            .queue
            .iter()
            .filter_map(|e| e.get("task_id").and_then(Value::as_str).map(str::to_string))
            .collect();
        let queue_urls: HashSet<String> = data
            .queue
            .iter()
            .filter_map(|e| e.get("url").and_then(Value::as_str).map(str::to_string))
            .collect();
        let retry_urls: HashSet<String> = data.retry_in_flight.iter().cloned().collect();
        let progress_active_urls: HashSet<String> = match &data.progress {
            Value::Object(m) => m
                .iter()
                .filter(|(_, v)| v.get("status").and_then(Value::as_str) == Some("Downloading"))
                .map(|(k, _)| k.clone())
                .collect(),
            _ => HashSet::new(),
        };
        let preserved: Vec<Value> = data
            .history
            .iter()
            .filter(|e| {
                is_active_history_entry(
                    e,
                    &queue_ids,
                    &queue_urls,
                    &retry_urls,
                    &progress_active_urls,
                )
            })
            .cloned()
            .collect();
        data.history = preserved.clone();
        state
            .dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        (preserved, data.history_path.clone())
    };
    persist_history(&preserved, &path);
    // F6: history SSE'i `broadcast_loop` (250ms) yayınlar.
    ok(contract::ok(Value::Null))
}

/// POST `/api/restart` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn restart(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/restart", &Value::Null).await {
            return ok(v);
        }
    }
    ok(contract::ok(
        json!({ "message": "Redémarrage en cours..." }),
    ))
}

/// POST `/api/support` — Faz 10c/3/4: `catalog` varsa Python'a proxy (zip), yoksa
/// saf-Rust modda gerçek redakteli support ZIP üretir (TASK-002-gap-13 / BELİRSİZ-2).
pub async fn support(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok((bytes, ct)) = c.post_binary("/api/support", &body).await {
            return (
                [
                    ("Content-Type", ct),
                    ("Access-Control-Allow-Origin", "*".to_string()),
                    (
                        "Content-Disposition",
                        "attachment; filename=rgsx_support.zip".to_string(),
                    ),
                ],
                bytes,
            )
                .into_response();
        }
    }
    // catalog-off: gerçek redakteli support zip üret (boş placeholder değil).
    let zip_bytes = build_support_zip(&state);
    (
        [
            ("Content-Type", "application/zip".to_string()),
            ("Access-Control-Allow-Origin", "*".to_string()),
            (
                "Content-Disposition",
                "attachment; filename=rgsx_support.zip".to_string(),
            ),
        ],
        zip_bytes,
    )
        .into_response()
}

/// TASK-002-gap-13: catalog-off modda `support()` için redakteli ZIP üretir.
///
/// Python `utils/security.generate_support_zip` parity'si: `rgsx_settings.json`
/// (redakte edilmiş), bilinen log dosyaları (ham), `README.txt`. Loglar yalnızca
/// `RGSX_LOGS_FOLDER` set ise eklenir (best-effort); redaksiyon yalnızca settings'e uygulanır
/// (Python ile aynı — loglar ham eklenir).
fn build_support_zip(state: &AppState) -> Vec<u8> {
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // rgsx_settings.json — redakte edilmiş ayarlar (AppState.data.settings).
    let settings_text = {
        let redacted = redact_secrets(&state.read().settings);
        serde_json::to_string_pretty(&redacted).unwrap_or_else(|_| "{}".to_string())
    };

    // README.txt içerik dosyası listesi.
    let logs_dir = std::env::var("RGSX_LOGS_FOLDER").ok();
    let mut included = vec!["rgsx_settings.json (redacted)".to_string()];
    if logs_dir.is_some() {
        included.push("RGSX.log (and related logs if present)".to_string());
    }
    included.push("README.txt".to_string());

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let readme = format!(
        "RGSX Support Package\nGenerated: {now} (unix epoch seconds)\n\nIncluded Files:\n{}\n\nSensitive values (passwords, API keys, tokens) are redacted.\nDO NOT share this file publicly.\n",
        included.iter().map(|f| format!("- {f}")).collect::<Vec<_>>().join("\n")
    );

    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));

    let _ = zip.start_file("rgsx_settings.json", opts);
    let _ = zip.write_all(settings_text.as_bytes());

    if let Some(ref dir) = logs_dir {
        for name in ["RGSX.log", "rgsx_web.log", "rgsx_web_startup.log"] {
            let path = std::path::Path::new(dir).join(name);
            if let Ok(contents) = std::fs::read(&path) {
                let _ = zip.start_file(name, opts);
                let _ = zip.write_all(&contents);
            }
        }
    }

    let _ = zip.start_file("README.txt", opts);
    let _ = zip.write_all(readme.as_bytes());

    let cursor = zip
        .finish()
        .unwrap_or_else(|_| std::io::Cursor::new(Vec::new()));
    cursor.into_inner()
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

/// POST `/api/shutdown` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn shutdown(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/shutdown", &Value::Null).await {
            return ok(v);
        }
    }
    // TASK-002-gap-1: devam eden tüm retry döngülerini (torrent + native DDL) uyandır.
    state.shutdown.notify_waiters();
    ok(contract::ok(Value::Null))
}

/// POST `/api/pause` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa bridge'e
/// `pause_all` (Gap-2). Body'de opsiyonel `task_id` verilirse yalnız o indirme
/// duraklatılır (Python `toggle_pause_download` karşılığı). Bridge yoksa placeholder.
pub async fn pause(State(state): State<AppState>, Json(body): Json<Option<Value>>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/pause", &Value::Null).await {
            return ok(v);
        }
    }
    let task_id = body
        .as_ref()
        .and_then(|b| b.get("task_id"))
        .and_then(Value::as_str);
    if let Some(bridge) = &state.bridge {
        match task_id {
            Some(id) => {
                let paused = match bridge.pause_torrent(id).await {
                    Ok(()) => 1,
                    Err(_) => 0,
                };
                return ok(contract::ok(json!({ "paused": paused })));
            }
            None => {
                // "Pause All": aktif torrentleri duraklat (pause_active) VE kuyruk
                // worker'ını dondur. Aksi halde `status` Running kalır, worker
                // `pending_set`'teki bekleyen öğeleri başlatmaya devam eder →
                // "Pause All" kuyruğu gerçekten durdurmaz (native yol parity'si).
                let paused = bridge.pause_all().await.unwrap_or(0);
                let mut d = state.write();
                state
                    .global_paused
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                d.status = QueueStatus::Paused;
                state
                    .dirty
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                for sig in d.pause_signals.values() {
                    sig.notify_one();
                }
                return ok(contract::ok(json!({ "paused": paused })));
            }
        }
    }
    // TASK-002-gap-29: native modda global pause — devam eden HTTP-direct
    // indirmeleri abort eder (pause_signals) ve yeni başlatmaları engeller.
    // F2 (gap-30): kuyruk durum makinesini `Paused`'a çeker → worker yeni
    // dispatch'i durdurur, gelen Add/AddBatch öğeleri `pending_set`'te buffer'lanır.
    let mut d = state.write();
    state
        .global_paused
        .store(true, std::sync::atomic::Ordering::Relaxed);
    d.status = QueueStatus::Paused;
    state
        .dirty
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let paused = d.pause_signals.len();
    for sig in d.pause_signals.values() {
        sig.notify_one();
    }
    ok(contract::ok(json!({ "paused": paused })))
}

/// POST `/api/resume` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa bridge'e
/// `resume_all` (Gap-2). Body'de opsiyonel `task_id` verilirse yalnız o indirme
/// sürdürülür. Bridge yoksa placeholder (0).
pub async fn resume(State(state): State<AppState>, Json(body): Json<Option<Value>>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/resume", &Value::Null).await {
            return ok(v);
        }
    }
    let task_id = body
        .as_ref()
        .and_then(|b| b.get("task_id"))
        .and_then(Value::as_str);
    if let Some(bridge) = &state.bridge {
        match task_id {
            Some(id) => {
                let resumed = match bridge.resume_torrent(id).await {
                    Ok(()) => 1,
                    Err(_) => 0,
                };
                return ok(contract::ok(json!({ "resumed": resumed })));
            }
            None => {
                // "Resume All": torrentleri sürdür (resume_active) VE kuyruk
                // worker'ını yeniden çalıştır. `status=Running` + `pending_notify`
                // ile bekleyen worker uyanır, `pending_set` drene edilir.
                let resumed = bridge.resume_all().await.unwrap_or(0);
                let mut d = state.write();
                state
                    .global_paused
                    .store(false, std::sync::atomic::Ordering::Relaxed);
                d.status = QueueStatus::Running;
                state
                    .dirty
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                d.pause_resume.notify_waiters();
                d.pending_notify.notify_waiters();
                return ok(contract::ok(json!({ "resumed": resumed })));
            }
        }
    }
    // TASK-002-gap-29: native modda global resume — duraklatılmış indirme
    // döngülerini uyandırır (pause_resume.notify_all).
    // F2 (gap-30): kuyruk durum makinesini `Running`'a çeker ve `pending_notify`
    // ile bekleyen worker'ı anında uyandırır → buffer'daki `pending_set` drene edilir.
    let mut d = state.write();
    state
        .global_paused
        .store(false, std::sync::atomic::Ordering::Relaxed);
    d.status = QueueStatus::Running;
    state
        .dirty
        .store(true, std::sync::atomic::Ordering::Relaxed);
    d.pause_resume.notify_waiters();
    d.pending_notify.notify_waiters();
    ok(contract::ok(json!({ "resumed": 0 })))
}

// ---------------------------------------------------------------------------
// TASK-013: /api/qbittorrent/* handler'ları (change_password, qb_start,
// qb_password_status, qb_regenerate_password) emekli edildi — librqbit tek
// torrent yolu; uçlar yalnız legacy python bridge altında anlamlıydı.
// ---------------------------------------------------------------------------

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
    (
        StatusCode::NOT_FOUND,
        [("Access-Control-Allow-Origin", "*")],
        "404 Not Found",
    )
        .into_response()
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

// ---------------------------------------------------------------------------
// İndirme sonuçlandırma yardımcıları (TASK-002f)
// ---------------------------------------------------------------------------

// --- TASK-002-gap-10 (A/B/D): history kalıcılık + clear koruma + timestamp ---

/// D: Python `add_to_history` timestamp formatı (`%Y-%m-%d %H:%M:%S`, yerel saat).
fn now_timestamp() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// B: Python `history.clear_history` `is_truly_active` parity'si — entry hâlâ
/// aktif bir indirmeyi mi temsil ediyor? (status + aktif id/url eşleşmesi)
fn is_active_history_entry(
    entry: &Value,
    queue_ids: &HashSet<String>,
    queue_urls: &HashSet<String>,
    retry_urls: &HashSet<String>,
    progress_active_urls: &HashSet<String>,
) -> bool {
    let status = entry.get("status").and_then(Value::as_str).unwrap_or("");
    let active_statuses = [
        "Downloading",
        "Téléchargement",
        "downloading",
        "Extracting",
        "Converting",
        "Queued",
        "Seeding",
    ];
    if !active_statuses.contains(&status) {
        return false;
    }
    let task_id = entry.get("task_id").and_then(Value::as_str).unwrap_or("");
    let url = entry.get("url").and_then(Value::as_str).unwrap_or("");
    if status == "Seeding" {
        return true;
    }
    if status == "Queued" {
        return queue_ids.contains(task_id) || queue_urls.contains(url);
    }
    queue_ids.contains(task_id)
        || queue_urls.contains(url)
        || retry_urls.contains(url)
        || progress_active_urls.contains(url)
}

/// A: geçerli history'yi (varsa) diske atomik yazar.
fn persist_history(history: &[Value], path: &Option<std::path::PathBuf>) {
    if let Some(p) = path {
        crate::persist::save_history(history, p);
    }
}

/// Arka plan indirme tamamlanınca state'i sonuçlandırır: history status'unu
/// `Download_OK`/`Erreur` yapar, kuyruktan çeker, başarılıysa `downloaded[platform]`
/// listesine ekler ve `queue`/`history`/`progress`(+`downloaded`) SSE yayınlar.
pub async fn finalize_download_in_state(
    state: &AppState,
    task_id: &str,
    game_url: &str,
    game_name: &str,
    platform: &str,
    ok: bool,
    message: &str,
) {
    let status = if ok { "Download_OK" } else { "Erreur" };
    let (history_snapshot, path) = {
        let mut data = state.write();
        let retries_for_url = data.retries.get(game_url).copied().unwrap_or(0);
        let retry_at_for_url = data.retry_at.get(game_url).copied();
        if let Some(entry) = data
            .history
            .iter_mut()
            .find(|e| e.get("task_id").and_then(Value::as_str) == Some(task_id))
        {
            entry["status"] = json!(status);
            entry["message"] = json!(message);
            entry["timestamp"] = json!(now_timestamp()); // D: tamamlanma zamanı
            if ok {
                entry["progress"] = json!(100);
                entry["entity_state"] = json!("COMPLETED");
            } else {
                entry["entity_state"] = json!("FAILED_PERMANENT");
                entry["error"] = json!(message);
            }
            entry["retry_count"] = json!(retries_for_url);
            entry["max_retries"] = json!(retry::DEFAULT_MAX_RETRIES);
            if let Some(ra) = retry_at_for_url {
                entry["retry_at"] = json!(ra);
            }
        }
        data.retries.remove(game_url);
        data.retry_at.remove(game_url);
        if let Some(pos) = data
            .queue
            .iter()
            .position(|e| e.get("task_id").and_then(Value::as_str) == Some(task_id))
        {
            data.queue.remove(pos);
        }
        if ok {
            if let Value::Object(map) = &mut data.downloaded {
                let list = map.entry(platform.to_string()).or_insert_with(|| json!([]));
                if let Some(arr) = list.as_array_mut() {
                    if !arr.iter().any(|g| g.as_str() == Some(game_name)) {
                        arr.push(json!(game_name));
                    }
                }
            }
            // F3-F4: O(1) "already downloaded?" indeksini artırımlı güncelle (batch
            // handler'ının O(N) array taraması yapmaması için).
            data.downloaded_index
                .insert((platform.to_string(), game_name.to_string()));
            // Faz 12.6e — indirilen dosyaya symlink oluştur (settings.symlink etkinse).
            apply_symlink(message);
        }
        // F3-F4: görev durumu tamamlandı olarak işaretle (O(1) `get_status`).
        // `set_task_state` eviction FIFO'sunu günceller ve `TASKS_CAP` tahliyesini tetikler.
        data.set_task_state(
            game_url.to_string(),
            if ok {
                TaskState::Completed
            } else {
                TaskState::Failed
            },
        );
        state
            .dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Value::Object(prog) = &mut data.progress {
            if ok {
                prog.insert(
                    game_url.to_string(),
                    json!({ "status": "Download_OK", "progress": 100 }),
                );
            } else {
                prog.insert(
                    game_url.to_string(),
                    json!({ "status": "Erreur", "message": message }),
                );
            }
        }
        // F6: queue/history/downloaded/progress SSE'i `broadcast_loop` (250ms) yayınlar.
        (data.history.clone(), data.history_path.clone())
    };
    persist_history(&history_snapshot, &path);
}

// --- TASK-002-gap-1: retry motoru yardımcıları (Python queue.py parity) ---

enum RetryDecision {
    Stop,
    Retry { new_task_id: String, delay: f64 },
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn retry_message(name: &str, count: u32, delay: f64) -> String {
    format!(
        "Retry {} (attempt {}/{}) in {}s",
        name,
        count,
        retry::DEFAULT_MAX_RETRIES,
        delay as u64
    )
}

fn push_queued_history_entry(
    state: &AppState,
    task_id: &str,
    url: &str,
    name: &str,
    platform: &str,
    status: &str,
    message: &str,
    retry_count: u32,
) {
    let (history_snapshot, path) = {
        let mut data = state.write();
        // TASK-002-gap-12: kuyruğa eklenen öğe henüz semaphore iznini edinmediyse
        // indirme yapmıyor; progress durumu "Queued" olmalı (gerçek transfer başlayınca
        // indirme döngüsü callback'i "Downloading"e günceller). Aksi halde N oyun
        // semaphore'da beklerken UI hepsini "Downloading" gösterir → "5 limit çalışmıyor"
        // yanılgısı (aktif transfer yine download_semaphore ile 5 ile sınırlı).
        data.progress[url] = json!({ "status": status, "progress": 0 });
        data.queue.push(json!({
            "url": url,
            "platform": platform,
            "game_name": name,
            "task_id": task_id,
            "status": "Queued",
        }));
        data.history.push(json!({
            "game_name": name,
            "platform": platform,
            "url": url,
            "status": status,
            "progress": 0,
            "message": message,
            "timestamp": now_timestamp(),
            "downloaded_size": 0,
            "total_size": 0,
            "task_id": task_id,
            "entity_state": if status == "Queued" { "QUEUED" } else { "RETRY_SCHEDULED" },
            "retry_count": retry_count,
            "max_retries": retry::DEFAULT_MAX_RETRIES,
            "retry_at": 0,
        }));
        // F6: queue/progress/history SSE'i `broadcast_loop` (250ms) yayınlar; `dirty`
        // bayrağını set et (idle daemon'da gereksiz serialization önlenir).
        state
            .dirty
            .store(true, std::sync::atomic::Ordering::Relaxed);
        (data.history.clone(), data.history_path.clone())
    };
    persist_history(&history_snapshot, &path);
}

fn classify_bridge_error(err: &BridgeError) -> ErrorClass {
    match err {
        BridgeError::Timeout(_) => ErrorClass::Transient,
        BridgeError::DiskSpace(_) | BridgeError::PermissionDenied(_) | BridgeError::Extract(_) => {
            ErrorClass::Permanent
        }
        _ => {
            let msg = err.to_string();
            retry::classify_error(&msg, None)
        }
    }
}

fn classify_download_error(err: &DownloadError) -> ErrorClass {
    match err {
        DownloadError::Network(_) => ErrorClass::Transient,
        DownloadError::Canceled => ErrorClass::Permanent,
        DownloadError::BrowserChallenge
        | DownloadError::HtmlInsteadOfPayload(_)
        | DownloadError::InvalidArchive
        | DownloadError::PartialArchiveRejected(_)
        | DownloadError::EmptyResponse(_)
        | DownloadError::InsufficientDiskSpace(_)
        | DownloadError::PermissionDenied(_) => ErrorClass::Permanent,
        DownloadError::Client(_) | DownloadError::Http(_) => {
            retry::classify_error(&err.message(), None)
        }
    }
}

/// TASK-002-gap-32: hafif bağlantı sondası. DNS gerektirmeyen iyi bilinen
/// IP'lere (8.8.8.8 / 1.1.1.1) TCP connect dener; herhangi biri açılırsa ağ
/// "geri döndü" kabul edilir. DNS literal kullanıldığından offline iken DNS
/// çözümlemesi takılmaz; connect 2s timeout ile sınırlıdır.
async fn probe_connectivity() -> bool {
    let p_t0 = std::time::Instant::now();
    eprintln!("[TRACE] probe_connectivity start");
    const PROBES: [&str; 2] = ["8.8.8.8:53", "1.1.1.1:53"];
    let mut result = false;
    for addr in PROBES {
        if tokio::time::timeout(Duration::from_secs(2), TcpStream::connect(addr))
            .await
            .map(|r| r.is_ok())
            .unwrap_or(false)
        {
            result = true;
            break;
        }
    }
    eprintln!(
        "[TRACE] probe_connectivity -> {} in {}ms",
        result,
        p_t0.elapsed().as_millis()
    );
    result
}

async fn decide_retry(
    state: &AppState,
    url: &str,
    name: &str,
    current_task_id: &str,
    err_msg: &str,
    err_class: ErrorClass,
    is_network: bool,
) -> RetryDecision {
    // TASK-002-gap-32: ağ-koptu tespiti — ardışık Network hatası sayacı.
    if is_network {
        // Ağ zaten aşağıysa ek retry budget yakma; loop-top park gate yakalar.
        if state.read().network_down.load(Ordering::Relaxed) {
            return RetryDecision::Retry {
                new_task_id: current_task_id.to_string(),
                delay: 0.0,
            };
        }
        let streak = {
            let data = state.write();
            data.network_error_streak.fetch_add(1, Ordering::SeqCst) + 1
        };
        if streak >= NETWORK_DOWN_THRESHOLD {
            let mut data = state.write();
            data.network_down.store(true, Ordering::SeqCst);
            eprintln!(
                "[TRACE] network_down -> TRUE (streak={}, url={})",
                streak, url
            );
            // Park edilecek indirmelerin durumunu "Ağ bekleniyor" yap (UI).
            for q in data.queue.iter_mut() {
                let s = q.get("status").and_then(Value::as_str).unwrap_or("");
                if s == "Downloading" || s == "Retrying" || s == "Connecting" || s == "Verifying" {
                    q["status"] = json!("Ağ bekleniyor");
                }
            }
            state.dirty.store(true, Ordering::SeqCst);
            // Sıfır gecikmeyle retry döngüsüne dön; loop-top park gate park eder.
            return RetryDecision::Retry {
                new_task_id: current_task_id.to_string(),
                delay: 0.0,
            };
        }
        // TASK-002-gap-32 (tek-indirme duyarlılığı): eşik altında ama gerçek bir
        // ağ hatası. Proaktif sonda çalıştır; ağ gerçekten erişilemezse tek
        // indirme (concurrency=1) senaryosunda da bayrağı çevir ki UI
        // "bağlantı kesildi" uyarısını göstersin. Sonda erişilebilirse (yalnızca
        // bu sunucu çöktü) yanlış pozitif olmaz, olağan retry'ye düşer.
        // Lock, async sonda öncesi bırakılır (await boyunca tutulmaz).
        if !probe_connectivity().await {
            let mut data = state.write();
            if !data.network_down.load(Ordering::SeqCst) {
                data.network_down.store(true, Ordering::SeqCst);
                eprintln!("[TRACE] network_down -> TRUE (probe, url={})", url);
                for q in data.queue.iter_mut() {
                    let s = q.get("status").and_then(Value::as_str).unwrap_or("");
                    if s == "Downloading"
                        || s == "Retrying"
                        || s == "Connecting"
                        || s == "Verifying"
                    {
                        q["status"] = json!("Ağ bekleniyor");
                    }
                }
                state.dirty.store(true, Ordering::SeqCst);
            }
            return RetryDecision::Retry {
                new_task_id: current_task_id.to_string(),
                delay: 0.0,
            };
        }
    } else {
        // Ağ-dışı hata → streak'i sıfırla (flapping'i önler).
        let data = state.write();
        data.network_error_streak.store(0, Ordering::SeqCst);
    }
    let mut data = state.write();
    let failures = *data.retries.get(url).unwrap_or(&0);
    if err_class == ErrorClass::Transient && failures < retry::DEFAULT_MAX_RETRIES {
        let new_failures = failures + 1;
        data.retries.insert(url.to_string(), new_failures);
        let delay = retry::retry_backoff_seconds(
            new_failures,
            retry::DEFAULT_BACKOFF_BASE_SEC,
            retry::DEFAULT_BACKOFF_MAX_SEC,
        );
        let retry_at = now_secs() + delay;
        data.retry_at.insert(url.to_string(), retry_at);
        // Faz A (retry aggregation): retry'ler YENİ satır açmaz, mevcut
        // parent görev satırı güncellenir — böylece Kuyruk/Geçmiş'te dosya başına
        // yalnızca TEK kayıt görünür (IDM/Aria2 davranışı).
        for e in data.history.iter_mut() {
            if e.get("task_id").and_then(Value::as_str) == Some(current_task_id) {
                e["status"] = json!("Téléchargement");
                e["entity_state"] = json!("RETRY_SCHEDULED");
                e["retry_count"] = json!(new_failures);
                e["max_retries"] = json!(retry::DEFAULT_MAX_RETRIES);
                e["retry_at"] = json!(retry_at);
                e["message"] = json!(retry_message(name, new_failures, delay));
            }
        }
        for q in data.queue.iter_mut() {
            if q.get("task_id").and_then(Value::as_str) == Some(current_task_id) {
                q["status"] = json!("Retrying");
                q["retry_count"] = json!(new_failures);
            }
        }
        // Aynı parent task_id korunur; retry döngüsü `current_task_id = new_task_id`
        // (artık eşit) ile devam ettiği için bir sonraki retry da aynı satırı bulur.
        RetryDecision::Retry {
            new_task_id: current_task_id.to_string(),
            delay,
        }
    } else {
        let _ = err_msg;
        RetryDecision::Stop
    }
}

/// Faz 12e — native DDL indirme: çözülmüş doğrudan HTTP kaynağını reqwest ile indirir,
/// `downloaded`/history/progress + SSE ile sonuçlanır (Python `one_fichier.py` DDL akışının
/// librqbit'siz karşılığı). Torrent kaynakları buraya gelmez (download() bunları bridge'e yönlendirir).
async fn native_ddl_download(
    state: AppState,
    game_url: String,
    resolved: String,
    platform: String,
    game_name: String,
) -> Response {
    let task_id = web_task_id();
    let downloads = if let Some(b) = &state.bridge {
        b.get_app_paths().await.map(|(d, _)| d).unwrap_or_default()
    } else {
        std::env::var("RGSX_DOWNLOADS_FOLDER").unwrap_or_else(|_| "downloads".to_string())
    };
    // Faz 12 download-parity: ROMS_FOLDER ayarlıysa dosya doğrudan
    // `ROMS_FOLDER/<platform>/<game>` altına düşer (Python queue.py davranışı).
    // Aksi halde geriye uyumlu `downloads_dir` kullanılır.
    let dest = {
        let roms_dest = match effective_roms_folder() {
            Some(rf) => rom_dest_for(&rf, &platform, &game_name, &game_url),
            None => dest_path_for(&downloads, &game_url, &game_name),
        };
        // gap-28: BIOS benzeri platformlar USERDATA_FOLDER'a yönlenir.
        redirect_bios_dest(roms_dest, &platform, &game_name)
    };

    push_queued_history_entry(
        &state,
        &task_id,
        &game_url,
        &game_name,
        &platform,
        "Queued",
        "Ajouté à la file d'attente (native DDL)",
        0,
    );

    // Closure'a taşınacak klonlar (yanıt `ok()` sahipleri kullanır).
    let c_state = state.clone();
    let c_url = game_url.clone();
    let c_name = game_name.clone();
    let c_plat = platform.clone();
    let c_task = task_id.clone();
    // TASK-002-gap-12 (dedup): aynı URL zaten aktifse yinelenen spawn'ı düşür.
    if !claim_in_flight(&c_state, &c_url) {
        return ok(contract::ok(json!({
            "queued": false,
            "message": "Déjà en cours de téléchargement",
            "url": c_url,
            "task_id": Value::Null,
        })));
    }
    let cancel: Arc<Notify> = {
        let mut d = c_state.write();
        let sig = Arc::new(Notify::new());
        d.cancel_signals.insert(c_url.clone(), sig.clone());
        sig
    };
    // TASK-002-gap-29: global pause abort sinyali (URL başına). Pause handler'ı
    // tüm aktif indirmelerin sinyalini tetikler → `CancelFlag` ile indirme durur.
    let pause_sig: Arc<Notify> = {
        let mut d = c_state.write();
        let sig = Arc::new(Notify::new());
        d.pause_signals.insert(c_url.clone(), sig.clone());
        sig
    };
    let shutdown = c_state.shutdown.clone();
    tokio::spawn(async move {
        let dl_t0 = std::time::Instant::now();
        eprintln!(
            "[TRACE] dl={} name={} spawned, waiting permit",
            c_task, c_name
        );
        // Faz 12.6d: eşzamanlı indirme sınırı (max_simultaneous_downloads).
        // `queue_clear` (stop-all) semaphore'u kapatınca `acquire` Err döner → görev
        // iptal edilir. Ayrıca `aborting` bayrağı geç klonlanan görevleri yakalar.
        let _dl_permit = {
            let sem = c_state.read().download_semaphore.clone();
            match sem.acquire_owned().await {
                Ok(p) => {
                    eprintln!(
                        "[TRACE] dl={} permit acquired ({}ms since spawn)",
                        c_task,
                        dl_t0.elapsed().as_millis()
                    );
                    p
                }
                Err(_) => {
                    let mut d = c_state.write();
                    d.retry_in_flight.remove(&c_url);
                    d.cancel_signals.remove(&c_url);
                    d.pause_signals.remove(&c_url);
                    return;
                }
            }
        };
        if c_state.read().aborting.load(Ordering::SeqCst) {
            let mut d = c_state.write();
            d.retry_in_flight.remove(&c_url);
            d.cancel_signals.remove(&c_url);
            d.pause_signals.remove(&c_url);
            return;
        }
        // Gap-4 4a — bellek içi `bytes()` yerine `HttpDownloader` stream motoru
        // (`.part` yazma, Range resume, challenge/HTML/arşiv guards, cancel).
        // TASK-002-gap-1: job-level retry envelope (torrent ile ortak).
        let mut current_task_id = c_task.clone();
        let current_url = c_url.clone();
        let mut aborted: Option<String> = None;
        loop {
            // TASK-002-gap-29: global pause aktifken yeni indirme başlamaz;
            // resume sinyaline kadar bekle (Python pause_all_downloads parity'si).
            if c_state
                .global_paused
                .load(std::sync::atomic::Ordering::Relaxed)
            {
                let pr = c_state.read().pause_resume.clone();
                // gap-30: missed-wakeup koruması. `notify_waiters()` bekleyen yokken
                // çağrılırsa notify KAYBOLUR → indirme sonsuza dek park kalır
                // (resume sonrası devam etmez, "indirme durdu" gözükür). 1s timeout
                // ile `global_paused` tekrar kontrol edilir; kaçırılan sinyal telafi
                // edilir, resume en geç 1s'de etkisini gösterir.
                let _ = tokio::time::timeout(Duration::from_millis(1000), pr.notified()).await;
                continue;
            }
            // TASK-002-gap-32: ağ koptuysa indirmeyi başlatmadan PARK et.
            let down = c_state.read().network_down.load(Ordering::Relaxed);
            if down {
                if probe_connectivity().await {
                    let mut d = c_state.write();
                    d.network_down.store(false, Ordering::SeqCst);
                    eprintln!(
                        "[TRACE] network_down -> FALSE (restored, url={})",
                        current_url
                    );
                    d.network_error_streak.store(0, Ordering::SeqCst);
                    for q in d.queue.iter_mut() {
                        if q.get("status").and_then(|v| v.as_str()) == Some("Ağ bekleniyor") {
                            q["status"] = json!("Downloading");
                        }
                    }
                    d.network_resume.notify_waiters();
                    c_state.dirty.store(true, Ordering::SeqCst);
                    // TASK-002-gap-32: yalnızca GERÇEK kesinti sonrası restore bildirimi.
                    if d.network_outage_confirmed
                        .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        crate::sse::publish(
                            &c_state.events,
                            "network_restored",
                            &serde_json::json!({ "network_down": false }),
                        );
                    }
                } else {
                    c_state
                        .write()
                        .network_outage_confirmed
                        .store(true, Ordering::SeqCst);
                    let nr = c_state.read().network_resume.clone();
                    let _ = tokio::time::timeout(Duration::from_millis(1000), nr.notified()).await;
                    continue;
                }
            }
            let progress_state = c_state.clone();
            let progress_url = current_url.clone();
            let req = manager_download::http::DownloadRequest {
                url: resolved.clone(),
                dest_path: dest.clone(),
                known_total_size: 0,
                referer: None,
                cookie: None,
            };
            let cancel_flag = CancelFlag::new();
            let cf = cancel_flag.clone();
            let first_pb = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let pb_t0 = dl_t0;
            let trace_task = c_task.clone();
            let downloader = manager_download::http::HttpDownloader::new()
                .with_cancel(cancel_flag)
                .with_retry(3, Duration::from_secs(5))
                .with_progress(move |downloaded, total| {
                    if !first_pb.swap(true, std::sync::atomic::Ordering::SeqCst) {
                        let p = if total > 0 {
                            (downloaded * 100 / total) as u32
                        } else {
                            0
                        };
                        eprintln!(
                            "[TRACE] dl={} first progress {}% ({}ms since spawn)",
                            trace_task,
                            p,
                            pb_t0.elapsed().as_millis()
                        );
                    }
                    let pct = if total > 0 {
                        (downloaded * 100 / total) as u32
                    } else {
                        0
                    };
                    let mut data = progress_state.write();
                    data.progress[&progress_url] =
                        json!({ "status": "Downloading", "progress": pct });
                    progress_state
                        .dirty
                        .store(true, std::sync::atomic::Ordering::Relaxed);
                    // F6: progress SSE'i artık `broadcast_loop` (250ms batched delta) yayınlar.
                });
            let dl_fut = downloader.download_async(&req);
            // TASK-002-gap-29: global pause (pause_sig) devam eden indirmeyi abort eder;
            // cancel/shutdown da aynı CancelFlag üzerinden keser.
            let mut paused_now = false;
            let result = tokio::select! {
                r = dl_fut => r,
                _ = pause_sig.notified() => {
                    cf.set();
                    paused_now = true;
                    Err(manager_download::http::DownloadError::Canceled)
                }
                _ = cancel.notified() => {
                    cf.set();
                    aborted = Some("İptal edildi".to_string());
                    Err(manager_download::http::DownloadError::Canceled)
                }
                _ = shutdown.notified() => {
                    cf.set();
                    aborted = Some("Sunucu kapatılıyor".to_string());
                    Err(manager_download::http::DownloadError::Canceled)
                }
            };

            match result {
                Ok(path) => {
                    c_state
                        .write()
                        .network_error_streak
                        .store(0, Ordering::SeqCst);
                    eprintln!(
                        "[TRACE] dl={} download OK, finalizing ({}ms since spawn)",
                        current_task_id,
                        dl_t0.elapsed().as_millis()
                    );
                    finalize_download_in_state(
                        &c_state,
                        &current_task_id,
                        &current_url,
                        &c_name,
                        &c_plat,
                        true,
                        &path.display().to_string(),
                    )
                    .await;
                    break;
                }
                Err(e) => {
                    // TASK-002-gap-29: global pause abort'u → loop başına dön;
                    // `global_paused` kontrolü resume'a kadar bekler. ÖNEMLİ:
                    // `current_task_id` DEĞİŞTİRİLMEZ. Queue/history satırı
                    // enqueue anındaki `task_id` ile etiketlenir; `finalize`
                    // (1686) o `task_id` ile eşleştirip öğeyi kuyruktan siler.
                    // Burada yeni `task_id` üretilirse eşleşme bozulur ve
                    // tamamlanan öğe kuyruktan silinmez → "kuyruk donuyor
                    // ama indirilenler artıyor" hatası (gap-31).
                    if paused_now {
                        continue;
                    }
                    let cls = classify_download_error(&e);
                    let is_network = matches!(e, DownloadError::Network(_));
                    match decide_retry(
                        &c_state,
                        &current_url,
                        &c_name,
                        &current_task_id,
                        &e.message(),
                        cls,
                        is_network,
                    )
                    .await
                    {
                        RetryDecision::Retry { new_task_id, delay } => {
                            let dur = Duration::from_secs_f64(delay.max(0.0));
                            tokio::select! {
                                _ = tokio::time::sleep(dur) => {}
                                _ = cancel.notified() => {
                                    cf.set();
                                    aborted = Some("İptal edildi".to_string());
                                }
                                _ = shutdown.notified() => {
                                    cf.set();
                                    aborted = Some("Sunucu kapatılıyor".to_string());
                                }
                            }
                            match aborted {
                                Some(ref msg) => {
                                    finalize_download_in_state(
                                        &c_state,
                                        &current_task_id,
                                        &current_url,
                                        &c_name,
                                        &c_plat,
                                        false,
                                        msg,
                                    )
                                    .await;
                                    break;
                                }
                                None => {
                                    current_task_id = new_task_id;
                                    continue;
                                }
                            }
                        }
                        RetryDecision::Stop => {
                            finalize_download_in_state(
                                &c_state,
                                &current_task_id,
                                &current_url,
                                &c_name,
                                &c_plat,
                                false,
                                &e.message(),
                            )
                            .await;
                            break;
                        }
                    }
                }
            }
        }
        let mut d = c_state.write();
        d.retry_in_flight.remove(&current_url);
        d.cancel_signals.remove(&current_url);
        d.pause_signals.remove(&current_url);
    });

    ok(contract::ok(json!({
        "queued": true,
        "game_name": game_name,
        "platform": platform,
        "task_id": task_id,
        "message": format!("{game_name} ajouté à la file d'attente (native DDL)"),
        "queue_position": 0,
    })))
}

/// TASK-002-gap-11 Faz5 — 1fichier native indirme (OF0..OF18 parity).
/// `native_ddl_download` ile aynı kuyruk/semaphore/pause/retry iskeleti, ancak
/// `manager_download::one_fichier::OneFichierOrchestrator` üzerinden debrid zinciri
/// (1F→AD→DL→RD→TB→FREE) ve FREE scrape akışını kullanır. Başarıda `provider_used`
/// history'ye yazılır (UI "AD:" parity).
async fn native_onefichier_download(
    state: AppState,
    game_url: String,
    platform: String,
    game_name: String,
) -> Response {
    let task_id = web_task_id();
    let downloads = if let Some(b) = &state.bridge {
        b.get_app_paths().await.map(|(d, _)| d).unwrap_or_default()
    } else {
        std::env::var("RGSX_DOWNLOADS_FOLDER").unwrap_or_else(|_| "downloads".to_string())
    };
    // DDL ile aynı dest_dir hesabı (roms/platform), filename zincirden gelir.
    let dest_dir = {
        let base_file = match effective_roms_folder() {
            Some(rf) => rom_dest_for(&rf, &platform, &game_name, &game_url),
            None => dest_path_for(&downloads, &game_url, &game_name),
        };
        let dir = base_file.parent().map(|p| p.to_path_buf()).unwrap_or(base_file);
        // gap-28 BIOS redirect (parent klasör seviyesinde)
        let file_probe = dir.join(sanitize_file_name(&game_name));
        let redirected = redirect_bios_dest(file_probe, &platform, &game_name);
        redirected.parent().map(|p| p.to_path_buf()).unwrap_or(dir)
    };

    push_queued_history_entry(
        &state,
        &task_id,
        &game_url,
        &game_name,
        &platform,
        "Queued",
        "Ajouté à la file d'attente (1fichier)",
        0,
    );

    let c_state = state.clone();
    let c_url = game_url.clone();
    let c_name = game_name.clone();
    let c_plat = platform.clone();
    let c_task = task_id.clone();
    if !claim_in_flight(&c_state, &c_url) {
        return ok(contract::ok(json!({
            "queued": false,
            "message": "Déjà en cours de téléchargement",
            "url": c_url,
            "task_id": Value::Null,
        })));
    }
    let cancel: Arc<Notify> = {
        let mut d = c_state.write();
        let sig = Arc::new(Notify::new());
        d.cancel_signals.insert(c_url.clone(), sig.clone());
        sig
    };
    let pause_sig: Arc<Notify> = {
        let mut d = c_state.write();
        let sig = Arc::new(Notify::new());
        d.pause_signals.insert(c_url.clone(), sig.clone());
        sig
    };
    let shutdown = c_state.shutdown.clone();
    let c_dir = dest_dir.clone();
    tokio::spawn(async move {
        let _dl_t0 = std::time::Instant::now();
        let _dl_permit = {
            let sem = c_state.read().download_semaphore.clone();
            match sem.acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    let mut d = c_state.write();
                    d.retry_in_flight.remove(&c_url);
                    d.cancel_signals.remove(&c_url);
                    d.pause_signals.remove(&c_url);
                    return;
                }
            }
        };
        if c_state.read().aborting.load(Ordering::SeqCst) {
            let mut d = c_state.write();
            d.retry_in_flight.remove(&c_url);
            d.cancel_signals.remove(&c_url);
            d.pause_signals.remove(&c_url);
            return;
        }
        let mut current_task_id = c_task.clone();
        let current_url = c_url.clone();
        let mut aborted: Option<String> = None;
        // 1fichier orchestrator (ApiKeys env/file)
        let keys = manager_download::one_fichier::ApiKeys::from_env();
        let orch = manager_download::one_fichier::OneFichierOrchestrator::new(keys);
        // Auto-extract ayarı
        let auto_extract = manager_core::settings::Settings::load().auto_extract;
        // Platform'dan is_zip_non_supported çıkarımı: şu an web katmanında bilgi yok → false
        // (queue.py'de platform'a göre hesaplanıyordu; native DDL'de torrent dışı için false).
        let is_zip_non_supported = false;
        loop {
            if c_state.global_paused.load(Ordering::Relaxed) {
                let pr = c_state.read().pause_resume.clone();
                let _ = tokio::time::timeout(Duration::from_millis(1000), pr.notified()).await;
                continue;
            }
            let down = c_state.read().network_down.load(Ordering::Relaxed);
            if down {
                if probe_connectivity().await {
                    let mut d = c_state.write();
                    d.network_down.store(false, Ordering::SeqCst);
                    d.network_error_streak.store(0, Ordering::SeqCst);
                    for q in d.queue.iter_mut() {
                        if q.get("status").and_then(|v| v.as_str()) == Some("Ağ bekleniyor") {
                            q["status"] = json!("Downloading");
                        }
                    }
                    d.network_resume.notify_waiters();
                    c_state.dirty.store(true, Ordering::SeqCst);
                    if d.network_outage_confirmed.compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
                        crate::sse::publish(&c_state.events, "network_restored", &json!({ "network_down": false }));
                    }
                } else {
                    c_state.write().network_outage_confirmed.store(true, Ordering::SeqCst);
                    let nr = c_state.read().network_resume.clone();
                    let _ = tokio::time::timeout(Duration::from_millis(1000), nr.notified()).await;
                    continue;
                }
            }
            let progress_state = c_state.clone();
            let progress_url = current_url.clone();
            let cf = CancelFlag::new();
            let cf2 = cf.clone();
            let on_progress: std::sync::Arc<manager_download::http::stream::ProgressCb> =
                std::sync::Arc::new(move |downloaded: u64, total: u64| {
                    let pct = if total > 0 { (downloaded * 100 / total) as u32 } else { 0 };
                    let mut data = progress_state.write();
                    data.progress[&progress_url] = json!({ "status": "Downloading", "progress": pct });
                    progress_state.dirty.store(true, Ordering::Relaxed);
                });
            let orch_fut = orch.download(&current_url, &c_dir, &c_name, &c_plat, is_zip_non_supported, auto_extract, Some(&cf2), Some(on_progress));
            let mut paused_now = false;
            let result = tokio::select! {
                r = orch_fut => r,
                _ = pause_sig.notified() => { cf.set(); paused_now = true; Err(DownloadError::Canceled) },
                _ = cancel.notified() => { cf.set(); aborted = Some("İptal edildi".to_string()); Err(DownloadError::Canceled) },
                _ = shutdown.notified() => { cf.set(); aborted = Some("Sunucu kapatılıyor".to_string()); Err(DownloadError::Canceled) },
            };
            match result {
                Ok((provider, path)) => {
                    c_state.write().network_error_streak.store(0, Ordering::SeqCst);
                    // Provider history parity (OF _set_provider_in_history)
                    {
                        let (used, prefix) = manager_download::one_fichier::history_provider_fields(provider);
                        c_state.dirty.store(true, Ordering::SeqCst);
                        let mut d = c_state.write();
                        for e in d.history.iter_mut() {
                            if e.get("url").and_then(|v| v.as_str()) == Some(&current_url) {
                                e["provider"] = json!(used);
                                e["provider_prefix"] = json!(prefix);
                            }
                        }
                        for q in d.queue.iter_mut() {
                            if q.get("url").and_then(|v| v.as_str()) == Some(&current_url) {
                                q["provider"] = json!(used);
                            }
                        }
                    }
                    finalize_download_in_state(&c_state, &current_task_id, &current_url, &c_name, &c_plat, true, &path.display().to_string()).await;
                    break;
                }
                Err(e) => {
                    if paused_now { continue; }
                    let cls = classify_download_error(&e);
                    let is_network = matches!(e, DownloadError::Network(_));
                    match decide_retry(&c_state, &current_url, &c_name, &current_task_id, &e.message(), cls, is_network).await {
                        RetryDecision::Retry { new_task_id, delay } => {
                            let dur = Duration::from_secs_f64(delay.max(0.0));
                            tokio::select! {
                                _ = tokio::time::sleep(dur) => {},
                                _ = cancel.notified() => { cf.set(); aborted = Some("İptal edildi".to_string()); },
                                _ = shutdown.notified() => { cf.set(); aborted = Some("Sunucu kapatılıyor".to_string()); },
                            }
                            match aborted {
                                Some(ref msg) => {
                                    finalize_download_in_state(&c_state, &current_task_id, &current_url, &c_name, &c_plat, false, msg).await;
                                    break;
                                }
                                None => { current_task_id = new_task_id; continue; }
                            }
                        }
                        RetryDecision::Stop => {
                            finalize_download_in_state(&c_state, &current_task_id, &current_url, &c_name, &c_plat, false, &e.message()).await;
                            break;
                        }
                    }
                }
            }
        }
        let mut d = c_state.write();
        d.retry_in_flight.remove(&current_url);
        d.cancel_signals.remove(&current_url);
        d.pause_signals.remove(&current_url);
    });

    ok(contract::ok(json!({
        "queued": true,
        "game_name": game_name,
        "platform": platform,
        "task_id": task_id,
        "message": format!("{game_name} ajouté à la file d'attente (1fichier)"),
        "queue_position": 0,
    })))
}

/// `get_app_paths` downloads klasörü + URL/oyun adından hedef dosya yolunu kurar.
/// URL'nin sondaki parçası bilinen bir ROM/.torrent uzantısıyla bitiyorsa onu,
/// değilse temizlenmiş `game_name`'i dosya adı yapar (Python
/// `check_extension_before_download` + `_build_download_path` niyeti).
pub fn dest_path_for(downloads_folder: &str, url: &str, game_name: &str) -> std::path::PathBuf {
    let base = std::path::PathBuf::from(downloads_folder);
    let fallback = sanitize_file_name(game_name);
    let from_url = url
        .split('/')
        .filter(|s| !s.is_empty())
        .next_back()
        .filter(|seg| known_torrent_extension(seg))
        .map(|s| s.to_string().replace("%20", " "))
        .unwrap_or(fallback);
    base.join(from_url)
}

/// `effective_roms_folder()` set edildiğinde indirme hedefi `ROMS/<platform>/<oyun>`
/// altına düşer. `game_name` çoğu zaman uzantısızdır; burada kaynak URL'in uzantısı
/// (.zip/.rar/.adf vb.) korunur ki RetroBat/ES dosyayı tanısın. Aksi halde
/// `RGSX_ROMS_FOLDER` set iken `dest_path_for`'un eklediği uzantı kaybolurdu (regresyon).
fn rom_dest_for(
    roms_folder: &std::path::Path,
    platform: &str,
    game_name: &str,
    url: &str,
) -> std::path::PathBuf {
    let sanitized = sanitize_file_name(game_name);
    let mut name = sanitized.clone();
    if std::path::Path::new(&sanitized).extension().is_none() {
        if let Some(seg) = url.split('/').filter(|s| !s.is_empty()).next_back() {
            if let Some(ext) = std::path::Path::new(seg).extension() {
                let ext = ext.to_string_lossy();
                if !ext.is_empty() {
                    name = format!("{}.{}", sanitized, ext);
                }
            }
        }
    }
    roms_folder.join(platform_folder_for(platform)).join(name)
}

/// Faz 12.6e — indirme tamamlandığında, `settings.symlink` etkinse nihai dosyayı
/// `symlink.target_directory` içine sembolik bağ (symlink) olarak oluşturur.
/// Unix/Windows için ayrı syscall; hedef dizin yoksa oluşturulur, mevcut link
/// varsa yenilenir. Hata sessizce yutulur (indirme başarısını etkilemez).
fn apply_symlink(src_path: &str) {
    let s = manager_core::settings::Settings::load();
    if !s.symlink.enabled {
        return;
    }
    let target = s.symlink.target_directory.trim();
    if target.is_empty() {
        return;
    }
    let src = std::path::Path::new(src_path);
    if !src.exists() {
        return;
    }
    let file_name = match src.file_name() {
        Some(n) => std::path::PathBuf::from(n),
        None => return,
    };
    let target_dir = std::path::Path::new(target);
    if let Err(_) = std::fs::create_dir_all(target_dir) {
        return;
    }
    let link = target_dir.join(file_name);
    let _ = std::fs::remove_file(&link);
    #[cfg(unix)]
    let res = std::os::unix::fs::symlink(src, &link);
    #[cfg(windows)]
    let res = std::os::windows::fs::symlink_file(src, &link);
    #[cfg(not(any(unix, windows)))]
    let res = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "symlink unsupported",
    ));
    if let Err(e) = res {
        tracing::warn!(
            "symlink oluşturulamadı ({} → {}): {}",
            src.display(),
            link.display(),
            e
        );
    }
}

/// TASK-002l: doğrudan çözülmüş bir URL'in torrent indirme şeması olup olmadığı.
/// `magnet:`, `rgsx+torrent:` ve `.torrent` (sorgu dizesi sonrası) kabul edilir.
/// Bu şemalar librqbit engine'ine yönlendirilir; diğer URL'ler (düz http dosya)
/// çözüm/katalog için Python'a proxy edilir.
fn is_torrent_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    if u.starts_with("magnet:") || u.starts_with("rgsx+torrent:") {
        return true;
    }
    // `.torrent` uzantısı — olası `?query` sonrasını çıkar.
    u.split('?').next().unwrap_or("").ends_with(".torrent")
}

/// 1fichier URL'i mi? (Python `is_1fichier_url` parity).
fn is_onefichier_url(url: &str) -> bool {
    url.to_ascii_lowercase().contains("1fichier.com")
}

/// Bilinen ROM / torrent dosya uzantısı (Python `check_extension_before_download`).
fn known_torrent_extension(seg: &str) -> bool {
    match std::path::Path::new(seg)
        .extension()
        .and_then(|e| e.to_str())
    {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            matches!(
                ext.as_str(),
                "torrent"
                    | "zip"
                    | "7z"
                    | "rar"
                    | "iso"
                    | "chd"
                    | "cue"
                    | "bin"
                    | "gdi"
                    | "nes"
                    | "snes"
                    | "smc"
                    | "gb"
                    | "gbc"
                    | "gba"
                    | "nds"
                    | "n64"
                    | "z64"
                    | "v64"
                    | "psp"
                    | "pbp"
                    | "cso"
                    | "img"
                    | "ccd"
                    | "m3u"
                    | "sv"
                    | "wbfs"
                    | "wad"
                    | "xci"
                    | "nsp"
            )
        }
        None => false,
    }
}

/// Dosya adı olarak kullanılacak metni temizler (path ayracı yasak).
fn sanitize_file_name(name: &str) -> String {
    name.replace(['/', '\\', ':'], "_")
}

/// Faz 12 download-parity: Python `resolve_platform_folder` (utils/files.py) eşiti.
/// `systems_list.json`'dan platform'un `folder`/`dossier` alanını bulur, yoksa
/// `normalize_platform_name` (lower + boşluk sil) fallback uygular.
fn platform_folder_for(platform: &str) -> String {
    let sources = std::env::var("RGSX_SOURCES_FILE").unwrap_or_default();
    let sources = if sources.is_empty() {
        let data = std::env::var("RGSX_DATA_DIR").unwrap_or_default();
        if data.is_empty() {
            String::new()
        } else {
            format!("{}/systems_list.json", data)
        }
    } else {
        sources
    };
    if !sources.is_empty() {
        if let Ok(txt) = std::fs::read_to_string(&sources) {
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&txt) {
                for e in &arr {
                    if e.get("platform_name").and_then(|v| v.as_str()) == Some(platform) {
                        if let Some(f) = e.get("folder").and_then(|v| v.as_str()) {
                            if !f.is_empty() {
                                return f.to_string();
                            }
                        }
                        if let Some(f) = e.get("dossier").and_then(|v| v.as_str()) {
                            if !f.is_empty() {
                                return f.to_string();
                            }
                        }
                    }
                }
            }
        }
    }
    platform.to_lowercase().replace(' ', "")
}

/// Faz 12 download-parity: efektif ROM kökü (env `RGSX_ROMS_FOLDER` > `settings.roms_folder`).
/// Boşsa `None` → geriye uyumlu `downloads_dir` davranışına düşülür.
fn effective_roms_folder() -> Option<std::path::PathBuf> {
    if let Ok(e) = std::env::var("RGSX_ROMS_FOLDER") {
        if !e.trim().is_empty() {
            return Some(std::path::PathBuf::from(e));
        }
    }
    let s = manager_core::settings::Settings::load();
    let rf = s.roms_folder.trim();
    if !rf.is_empty() {
        return Some(std::path::PathBuf::from(rf));
    }
    None
}

/// `USERDATA_FOLDER` eşdeğeri (Python `config.USERDATA_FOLDER` parity).
///
/// Çözüm önceliği: `RGSX_USERDATA_FOLDER` env > `RGSX_DATA_DIR`'dan 3 seviye
/// yukarı > `RGSX_ROMS_FOLDER`'dan 1 seviye yukarı. Hiçbiri yoksa `None`
/// (BIOS yönlendirmesi atlanır, roms alt klasörü kullanılır — geriye uyumlu).
///
/// Python: `USERDATA_FOLDER = dirname(dirname(dirname(APP_FOLDER)))` →
/// Linux/Batocera'da `/userdata`, Windows'ta kurulum kökünden 3 seviye yukarı.
fn userdata_folder() -> Option<std::path::PathBuf> {
    if let Ok(p) = std::env::var("RGSX_USERDATA_FOLDER") {
        if !p.trim().is_empty() {
            return Some(std::path::PathBuf::from(p));
        }
    }
    if let Ok(d) = std::env::var("RGSX_DATA_DIR") {
        if !d.trim().is_empty() {
            if let Some(ud) = std::path::PathBuf::from(d).ancestors().nth(3) {
                return Some(ud.to_path_buf());
            }
        }
    }
    if let Ok(r) = std::env::var("RGSX_ROMS_FOLDER") {
        if !r.trim().is_empty() {
            if let Some(parent) = std::path::PathBuf::from(r).parent() {
                return Some(parent.to_path_buf());
            }
        }
    }
    None
}

/// BIOS benzeri platformlar (ör. "- BIOS by TMCTV -") için indirme/çıkarma
/// hedefini `userdata_folder()` eşdeğerine yönlendirir (Python `queue.py:770`
/// parity). `userdata_folder()` yoksa (env tanımsız) redirect atlanır.
///
/// Not: Rust'ta eskiden USERDATA_FOLDER kavramı yoktu; BIOS zipleri roms alt
/// klasörüne açılıyordu. Bu fonksiyon o açığı kapatır. Çıkarma hedefi zaten
/// `dest_path.parent()` olduğundan, dest_path USERDATA'ya kaydırılınca çıkarma
/// da otomatik olarak USERDATA'ya düşer.
fn redirect_bios_dest(
    roms_dest: std::path::PathBuf,
    platform: &str,
    game_name: &str,
) -> std::path::PathBuf {
    if manager_core::extract::is_bios_platform(&platform_folder_for(platform), platform) {
        if let Some(ud) = userdata_folder() {
            return ud.join(sanitize_file_name(game_name));
        }
    }
    roms_dest
}

/// TASK-005 — ES (EmulationStation) gamepad map'ini sunar.
///
/// RetroBat/Batocera'daki `es_input.cfg`'yi okur; RGSX UI'ın aynı fiziksel
/// tuşları ES ile paylaşmasını sağlar (ikinci remap gerekmez). Bulunamazsa
/// `{"found": false}` döner (webui varsayılan standart mapping'e düşer).
pub async fn es_input(State(_state): State<AppState>) -> Response {
    match crate::es_input::load_best() {
        Some(c) => {
            let mut rgsx = serde_json::Map::new();
            for action in c.actions.keys() {
                if let Some(idx) = crate::es_input::es_action_to_gamepad_index(action) {
                    rgsx.insert(action.clone(), json!(idx));
                }
            }
            ok(json!({
                "found": true,
                "deviceName": c.device_name,
                "guid": c.guid,
                "actions": c.actions,
                "rgsx": rgsx,
            }))
        }
        None => ok(json!({ "found": false })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::sync::Mutex;

    /// `RGSX_USERDATA_FOLDER` process-global env değişkenine bağımlı testleri
    /// serialize eder; paralel `#[test]` çalıştırmasında env yarışını (flakylik)
    /// önler.
    static USERDATA_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn now_timestamp_non_empty_and_formatted() {
        let ts = now_timestamp();
        assert!(!ts.is_empty());
        // "YYYY-MM-DD HH:MM:SS" — en az 19 karakter, iki tire.
        assert!(ts.len() >= 19);
        assert_eq!(ts.chars().filter(|c| *c == '-').count(), 2);
    }

    #[test]
    fn active_entry_preserved_by_status_and_queued() {
        let queue_ids: HashSet<String> = ["t1".to_string()].into_iter().collect();
        let queue_urls: HashSet<String> = ["http://q".to_string()].into_iter().collect();
        let retry_urls: HashSet<String> = HashSet::new();
        let prog: HashSet<String> = HashSet::new();

        let downloading = json!({ "status": "Downloading", "task_id": "t1", "url": "http://x" });
        assert!(is_active_history_entry(
            &downloading,
            &queue_ids,
            &queue_urls,
            &retry_urls,
            &prog
        ));

        let queued_active = json!({ "status": "Queued", "task_id": "t1", "url": "http://x" });
        assert!(is_active_history_entry(
            &queued_active,
            &queue_ids,
            &queue_urls,
            &retry_urls,
            &prog
        ));

        // Queued ama ne kuyrukta ne de aktif → korunmaz.
        let queued_orphan =
            json!({ "status": "Queued", "task_id": "nope", "url": "http://orphan" });
        assert!(!is_active_history_entry(
            &queued_orphan,
            &queue_ids,
            &queue_urls,
            &retry_urls,
            &prog
        ));

        // Tamamlanmış → korunmaz.
        let done = json!({ "status": "Download_OK", "task_id": "t1", "url": "http://x" });
        assert!(!is_active_history_entry(
            &done,
            &queue_ids,
            &queue_urls,
            &retry_urls,
            &prog
        ));

        // Seeding her zaman korunur.
        let seeding = json!({ "status": "Seeding", "task_id": "x", "url": "http://y" });
        assert!(is_active_history_entry(
            &seeding,
            &queue_ids,
            &queue_urls,
            &retry_urls,
            &prog
        ));
    }

    #[test]
    fn gap28_bios_redirects_dest_to_userdata() {
        let _env_guard = USERDATA_ENV_LOCK.lock().unwrap();
        let ud = std::env::temp_dir().join("rgsx_ud_gap28");
        std::fs::create_dir_all(&ud).unwrap();
        std::env::set_var("RGSX_USERDATA_FOLDER", &ud);
        let roms_dest = std::path::PathBuf::from("/roms/biosbytmctv/game.zip");
        // "- BIOS by TMCTV -" BIOS_LIKE kümesinde → USERDATA'ya yönlenir (queue.py:770 parity).
        let out = redirect_bios_dest(roms_dest.clone(), "- BIOS by TMCTV -", "game.zip");
        assert_eq!(out, ud.join("game.zip"));
        assert!(!out.starts_with("/roms"));
        let _ = std::fs::remove_dir_all(&ud);
    }

    #[test]
    fn gap28_non_bios_keeps_roms_dest() {
        let _env_guard = USERDATA_ENV_LOCK.lock().unwrap();
        let ud = std::env::temp_dir().join("rgsx_ud_gap28b");
        std::fs::create_dir_all(&ud).unwrap();
        std::env::set_var("RGSX_USERDATA_FOLDER", &ud);
        let roms_dest = std::path::PathBuf::from("/roms/snes/game.zip");
        let out = redirect_bios_dest(roms_dest.clone(), "Super Nintendo", "game.zip");
        assert_eq!(out, roms_dest);
        let _ = std::fs::remove_dir_all(&ud);
    }

    #[tokio::test]
    async fn gap12_claim_in_flight_dedups_same_url() {
        let state = AppState::empty();
        let url = "http://example.com/game.zip";
        // İlk claim başarılı.
        assert!(claim_in_flight(&state, url));
        // Aynı URL ikinci kez claim edilemez (çift spawn engellenir).
        assert!(!claim_in_flight(&state, url));
        // Farklı URL serbest.
        assert!(claim_in_flight(&state, "http://example.com/other.zip"));
        // Tamamlanma sonrası (retry_in_flight temizlenirse) tekrar claim edilebilir.
        state.write().retry_in_flight.remove(url);
        assert!(claim_in_flight(&state, url));
    }

    #[tokio::test]
    async fn gap12_queued_progress_status_is_queued_not_downloading() {
        let state = AppState::empty();
        let url = "http://example.com/queued.zip";
        // Kuyruğa eklenen öğe semaphore iznini henüz etmedi → progress "Downloading"
        // değil "Queued" olmalı (aksi halde N oyun beklerken UI hepsini indiriliyor
        // gösterir, "5 limit çalışmıyor" yanılgısı).
        push_queued_history_entry(
            &state,
            "task-1",
            url,
            "Queued Game",
            "SNES",
            "Queued",
            "Ajouté à la file d'attente",
            0,
        );
        let prog = state.read();
        let entry = prog
            .progress
            .get(url)
            .expect("queued entry in progress map");
        assert_eq!(entry["status"], "Queued");
        assert_eq!(entry["progress"], 0);
        // Aynı anda kuyrukta görünür.
        assert!(prog.queue.iter().any(|q| q["url"] == url));
    }

    #[tokio::test]
    async fn gap29_global_pause_flags_and_signals() {
        let state = AppState::empty();
        assert!(!state
            .global_paused
            .load(std::sync::atomic::Ordering::Relaxed));
        assert!(state.read().pause_signals.is_empty());

        // pause handler (native): global_paused=true + her aktif indirme sinyali tetiklenir.
        let sig: std::sync::Arc<tokio::sync::Notify> =
            std::sync::Arc::new(tokio::sync::Notify::new());
        {
            let mut d = state.write();
            state
                .global_paused
                .store(true, std::sync::atomic::Ordering::Relaxed);
            d.pause_signals
                .insert("http://x/game.zip".to_string(), sig.clone());
            sig.notify_one();
        }
        assert!(state
            .global_paused
            .load(std::sync::atomic::Ordering::Relaxed));

        // resume handler (native): global_paused=false + beklerenler uyandırılır.
        {
            let d = state.write();
            state
                .global_paused
                .store(false, std::sync::atomic::Ordering::Relaxed);
            d.pause_resume.notify_waiters();
        }
        assert!(!state
            .global_paused
            .load(std::sync::atomic::Ordering::Relaxed));
    }

    #[tokio::test]
    async fn gap29_paused_loop_top_blocks_until_resume() {
        // native_ddl_download loop-top global pause kontrolünü taklit eder:
        // global_paused iken indirme başlamaz, resume sonrası devam eder.
        let state = AppState::empty();
        state
            .global_paused
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let s = state.clone();
        let handle = tokio::spawn(async move {
            // loop başı: pause kontrolü
            if s.global_paused.load(std::sync::atomic::Ordering::Relaxed) {
                let pr = s.read().pause_resume.clone();
                pr.notified().await;
            }
            assert!(!s.global_paused.load(std::sync::atomic::Ordering::Relaxed));
        });
        // görev beklerken kısa süre bekle
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;
        {
            let d = state.write();
            state
                .global_paused
                .store(false, std::sync::atomic::Ordering::Relaxed);
            d.pause_resume.notify_waiters();
        }
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn failed_native_ddl_is_removed_from_queue() {
        // TASK-002-gap-fail: hatalı/başarısız indirme kuyrukta asılı kalmamalı.
        // native_ddl_download bir dead-host URL ile çağrılır; indirme hatası
        // (Network → Transient) retry envelope'i tükettikten sonra Stop'a düşer ve
        // finalize_download_in_state(ok=false) kuyruk kaydını task_id ile siler.
        let state = AppState::empty();
        let dead = "http://127.0.0.1:1/dead.zip".to_string();
        let _ = native_ddl_download(
            state.clone(),
            dead.clone(),
            dead.clone(),
            "Test".to_string(),
            "Dead Game".to_string(),
        )
        .await;

        // Öğe önce kuyruğa girdi (takip ediliyor, kaybolmadı).
        assert!(
            state
                .read()
                .queue
                .iter()
                .any(|q| q["game_name"] == "Dead Game"),
            "failed item should be enqueued first"
        );

        // Retry'ler tükendikten sonra kuyruktan silinmeli (asılı kalmamalı).
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(120);
        loop {
            if state.read().queue.is_empty() {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                panic!("FAILED: başarısız indirme kuyrukta asılı kaldı (120s sonra hâlâ mevcut)");
            }
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }

        // Geçmişte "Erreur" (kalıcı hata) olarak işaretlenmiş olmalı.
        let hist = state.read();
        assert!(
            hist.history
                .iter()
                .any(|e| e["game_name"] == "Dead Game" && e["status"] == "Erreur"),
            "history must record the failed download as Erreur"
        );
    }

    #[tokio::test]
    async fn gap32_network_streak_flips_down_and_parks_status() {
        // TASK-002-gap-32: 3 ardışık Network hatası → network_down=true ve
        // aktif kuyruk öğesi "Ağ bekleniyor" durumuna geçer (park edilir).
        let state = AppState::empty();
        {
            let mut d = state.write();
            d.queue
                .push(json!({"task_id":"t1","status":"Downloading","game_name":"x","url":"u"}));
        }
        let cls = classify_download_error(&DownloadError::Network("simulated".to_string()));
        assert_eq!(cls, ErrorClass::Transient);
        // İlk iki hata: henüz aşağı değil, sıradan Retry döner.
        for _ in 0..2 {
            let r = decide_retry(&state, "u", "x", "t1", "sim", cls, true).await;
            assert!(matches!(r, RetryDecision::Retry { .. }));
            assert!(!state.read().network_down.load(Ordering::Relaxed));
        }
        // Üçüncü hata: ağ aşağıya geçer.
        let r = decide_retry(&state, "u", "x", "t1", "sim", cls, true).await;
        assert!(matches!(r, RetryDecision::Retry { .. }));
        assert!(state.read().network_down.load(Ordering::Relaxed));
        let s = state.read().queue[0]
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap()
            .to_string();
        assert_eq!(s, "Ağ bekleniyor");
    }

    #[tokio::test]
    async fn gap32_non_network_error_resets_streak() {
        // Ağ-dışı hata, birikmiş streak'i sıfırlar (flapping'i önler).
        let state = AppState::empty();
        let ncls = classify_download_error(&DownloadError::Network("sim".to_string()));
        for _ in 0..2 {
            let _ = decide_retry(&state, "u", "x", "t1", "sim", ncls, true).await;
        }
        assert_eq!(state.read().network_error_streak.load(Ordering::Relaxed), 2);
        let pcls = classify_download_error(&DownloadError::Canceled);
        let _ = decide_retry(&state, "u", "x", "t1", "cancelled", pcls, false).await;
        assert_eq!(state.read().network_error_streak.load(Ordering::Relaxed), 0);
    }
}
