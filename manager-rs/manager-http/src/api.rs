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
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::{json, Value};
use tokio::sync::Notify;

use manager_core::contract;
use manager_core::retry::{self, ErrorClass};
use manager_core::secrets::redact_secrets;
use manager_core::state::ManagerState;
use manager_bridge::BridgeError;
use manager_download::http::stream::CancelFlag;
use manager_download::http::DownloadError;

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
pub async fn search(State(state): State<AppState>, Query(params): Query<std::collections::HashMap<String, String>>) -> Response {
    let term = params.get("q").cloned().unwrap_or_default();
    if let Some(c) = &state.catalog {
        let route = format!("/api/search?q={}", percent_encoding::utf8_percent_encode(&term, percent_encoding::NON_ALPHANUMERIC));
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
pub async fn translations(Query(params): Query<HashMap<String, String>>, State(state): State<AppState>) -> Response {
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
pub async fn games(State(state): State<AppState>, AxumPath(platform): AxumPath<String>) -> Response {
    if let Some(c) = &state.catalog {
        let route = format!("/api/games/{}", percent_encoding::utf8_percent_encode(&platform, percent_encoding::NON_ALPHANUMERIC));
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
            entry
        })
        .collect();
    ok(contract::ok(json!({ "count": cleaned.len(), "history": cleaned })))
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
    ok(contract::ok(json!({
        "active": data.active,
        "queue": data.queue,
        "queue_size": data.queue_size(),
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

/// GET `/api/game-status` — Faz 10c/3/3: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn game_status(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
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
    let base = requested.cloned().unwrap_or_else(|| {
        match std::env::var("RGSX_DATA_DIR") {
            Ok(d) if std::path::Path::new(&d).is_dir() => d,
            _ => ".".to_string(),
        }
    });
    let (current, dirs) = state.read().browse(&base);
    ok(contract::ok(json!({ "current_path": current, "directories": dirs })))
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
pub async fn image(State(state): State<AppState>, AxumPath(platform): AxumPath<String>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok((bytes, ct)) = c.get_image(&platform).await {
            return (
                [("Content-Type", ct), ("Access-Control-Allow-Origin", "*".to_string())],
                bytes,
            )
                .into_response();
        }
    }
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
/// (rgsx_manager.py:337) ile birebir; başarı `Queued` + arka plan indirme.
///
/// Bridge (librqbit engine) varsa url+game_name gerçekten indirmeye başlar:
/// `downloads_folder` + türetilmiş dosya adıyla `download_torrent` arka plan
/// task'ında koşar, bitince history/downloaded/progress + SSE sonuçlanır.
/// Bridge yoksa (pure placeholder) eski davranış korunur — yalnız kuyruklanır.
///
/// Not: tüm `.await`'ler `state.write()` kilidinden ÖNCE — write guard sonrası
/// await handler future'ını Send yapmaz (bkz. change_password deseni).
pub async fn download(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    // TASK-002l: doğrudan çözülmüş torrent URL'i (magnet:/rgsx+torrent:/.torrent) ve
    // bir bridge (librqbit varsayılan) mevcutsa, Python catalog'a proxy ETME — indirme
    // engine'e yönlendirilir (canlıda catalog olsa bile). Aksi halde mevcut davranış:
    // `catalog` varsa game_index/game_name çözümü için Python'a proxy edilir.
    let direct_url = body.get("url").and_then(Value::as_str);
    let intercept_locally = direct_url.map(is_torrent_url).unwrap_or(false) && state.bridge.is_some();

    // Faz 12e: native DDL çözümü + doğrudan HTTP indirme. Yalnız `RGSX_NATIVE_DOWNLOAD=1`
    // ile; debrid yapılandırılmamışsa `DownloadManager` DirectResolver'a düşer ve düz
    // HTTP kaynak doğrudan indirilir. Kapalıyken mevcut Python proxy korunur.
    // gap-27: saf-Rust varsayılan = true (native DDL açık). Flag yine env ile override edilebilir.
    if std::env::var("RGSX_NATIVE_DOWNLOAD").map(|v| v == "1").unwrap_or(true) {
        if let Some(direct) = direct_url {
            if !is_torrent_url(direct) {
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
                    return native_ddl_download(state, direct.to_string(), resolved, platform, game_name).await;
                }
            }
        }
    }

    if !intercept_locally {
        // Faz 10c/3/4: `catalog` varsa Python'a proxy (game_index/game_name çözümü Python'da).
        // Placeholder'da WebUI yalnızca game_index gönderir — yerel `direct_url` yolu bridge/librqbit içindir.
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
        // Katalog yoksa (native mod) doğrudan HTTP indirme yapılır.
        if !is_torrent_url(&game_url) {
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
            _ => dest_path_for(&downloads, &game_url, &gname),
        };
            let state2 = state.clone();
            let u = game_url.clone();
            let n = gname.clone();
            let p = platform.clone();
            let t = task_id.clone();
            // TASK-002-gap-1: retry döngüsü (torrent + native DDL ortak envelope).
            // Her transient başarısızlıkta yeni task_id + yeni history entry
            // (Python queue.py:610 parity). retry_in_flight dedup + cancel/shutdown.
            {
                let mut d = state.write();
                d.retry_in_flight.insert(game_url.clone());
            }
            let cancel: Arc<Notify> = {
                let mut d = state.write();
                let sig = Arc::new(Notify::new());
                d.cancel_signals.insert(game_url.clone(), sig.clone());
                sig
            };
            let shutdown = state.shutdown.clone();
            tokio::spawn(async move {
                let mut current_task_id = t.clone();
                let current_url = u.clone();
                let mut aborted: Option<String> = None;
                loop {
                    let cb_state = state2.clone();
                    let cb_url = current_url.clone();
                    let on_progress: Option<Arc<dyn Fn(manager_bridge::ProgressEvent) + Send + Sync>> =
                        Some(Arc::new(move |ev: manager_bridge::ProgressEvent| {
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
                            sse::publish(&cb_state.events, "progress", &json!(data.progress));
                        }));
                    let res = bridge
                        .download_torrent_progress(&current_url, &dest_path, Some(current_task_id.clone()), on_progress)
                        .await;
                    match res {
                        Ok(src) => {
                            finalize_download_in_state(&state2, &current_task_id, &current_url, &n, &p, true, src.to_string_lossy().as_ref()).await;
                            break;
                        }
                        Err(e) => {
                            let cls = classify_bridge_error(&e);
                            match decide_retry(&state2, &current_url, &n, &p, &current_task_id, &e.to_string(), cls) {
                                RetryDecision::Retry { new_task_id, delay } => {
                                    let dur = Duration::from_secs_f64(delay.max(0.0));
                                    tokio::select! {
                                        _ = tokio::time::sleep(dur) => {}
                                        _ = cancel.notified() => { aborted = Some("İptal edildi".to_string()); }
                                        _ = shutdown.notified() => { aborted = Some("Sunucu kapatılıyor".to_string()); }
                                    }
                                    match aborted {
                                        Some(ref msg) => {
                                            finalize_download_in_state(&state2, &current_task_id, &current_url, &n, &p, false, msg).await;
                                            break;
                                        }
                                        None => { current_task_id = new_task_id; continue; }
                                    }
                                }
                                RetryDecision::Stop => {
                                    finalize_download_in_state(&state2, &current_task_id, &current_url, &n, &p, false, &e.to_string()).await;
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
    json_err(format!("Index de jeu invalide: {idx}"), StatusCode::BAD_REQUEST)
}

/// POST `/api/download/batch` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa 400.
pub async fn download_batch(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/download/batch", &body).await {
            return ok(v);
        }
    }
    json_err("Batch indirme devre dışı (RGSX_PYTHON_MANAGER_URL gerekli)", StatusCode::BAD_REQUEST)
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

/// POST `/api/queue/clear` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa yerel.
pub async fn queue_clear(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/queue/clear", &Value::Null).await {
            return ok(v);
        }
    }
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

/// POST `/api/queue/remove` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa yerel.
pub async fn queue_remove(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/queue/remove", &body).await {
            return ok(v);
        }
    }
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
            Ok(s) => match s.validate() {
                Ok(()) => match s.save() {
                    Ok(()) => return ok(contract::ok(Value::Null)),
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
            },
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

/// POST `/api/save_filters` — Faz 10c/3/3: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn save_filters(State(state): State<AppState>, Json(body): Json<Value>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/save_filters", &body).await {
            return ok(v);
        }
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
                is_active_history_entry(e, &queue_ids, &queue_urls, &retry_urls, &progress_active_urls)
            })
            .cloned()
            .collect();
        data.history = preserved.clone();
        (preserved, data.history_path.clone())
    };
    persist_history(&preserved, &path);
    sse::publish(&state.events, "history", &json!(preserved));
    ok(contract::ok(Value::Null))
}

/// POST `/api/restart` — Faz 10c/3/4: `catalog` varsa Python'a proxy, yoksa placeholder.
pub async fn restart(State(state): State<AppState>) -> Response {
    if let Some(c) = &state.catalog {
        if let Ok(v) = c.post_json("/api/restart", &Value::Null).await {
            return ok(v);
        }
    }
    ok(contract::ok(json!({ "message": "Redémarrage en cours..." })))
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
                    ("Content-Disposition", "attachment; filename=rgsx_support.zip".to_string()),
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
            ("Content-Disposition", "attachment; filename=rgsx_support.zip".to_string()),
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

    let cursor = zip.finish().unwrap_or_else(|_| std::io::Cursor::new(Vec::new()));
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
    let task_id = body.as_ref().and_then(|b| b.get("task_id")).and_then(Value::as_str);
    if let Some(bridge) = &state.bridge {
        let paused = match task_id {
            Some(id) => match bridge.pause_torrent(id).await {
                Ok(()) => 1,
                Err(_) => 0,
            },
            None => bridge.pause_all().await.unwrap_or(0),
        };
        return ok(contract::ok(json!({ "paused": paused })));
    }
    let paused = state.read().queue_size();
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
    let task_id = body.as_ref().and_then(|b| b.get("task_id")).and_then(Value::as_str);
    if let Some(bridge) = &state.bridge {
        let resumed = match task_id {
            Some(id) => match bridge.resume_torrent(id).await {
                Ok(()) => 1,
                Err(_) => 0,
            },
            None => bridge.resume_all().await.unwrap_or(0),
        };
        return ok(contract::ok(json!({ "resumed": resumed })));
    }
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

/// POST `/api/qbittorrent/regenerate-password` — bridge'e `regenerate_qbittorrent_password`.
/// Python 1:1: `(ok, password)` → `{"success": True, "password": pw}`; başarısız 500.
pub async fn qb_regenerate_password(State(state): State<AppState>) -> Response {
    match state
        .bridge_call("regenerate_qbittorrent_password", json!({}))
        .await
    {
        Ok(v) => {
            let arr = v.as_array();
            let ok_flag = arr.and_then(|a| a.first()).and_then(Value::as_bool).unwrap_or(false);
            let pw = arr
                .and_then(|a| a.get(1))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            if !ok_flag {
                return cors_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    json!({ "success": false, "message": "password_regeneration_failed" }),
                );
            }
            ok(contract::ok(json!({ "password": pw })))
        }
        Err(_) => cors_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "success": false, "message": "bridge_unavailable" }),
        ),
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
    }
    if let Value::Object(prog) = &mut data.progress {
        if ok {
            prog.insert(game_url.to_string(), json!({ "status": "Download_OK", "progress": 100 }));
        } else {
            prog.insert(game_url.to_string(), json!({ "status": "Erreur", "message": message }));
        }
    }
    sse::publish(&state.events, "queue", &json!({ "queue": data.queue, "active": data.active }));
    sse::publish(&state.events, "history", &json!(data.history));
    if ok {
        sse::publish(&state.events, "downloaded", &json!(data.downloaded));
    }
    sse::publish(&state.events, "progress", &json!(data.progress));
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

fn retry_task_id(url: &str) -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut h: u64 = 1469598103934665603;
    for b in url.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    let hash = (h & 0xFFFFFF) as u32;
    format!("retry_{}_{:06x}", millis, hash)
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
    data.progress[url] = json!({ "status": "Downloading", "progress": 0 });
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
    sse::publish(
        &state.events,
        "queue",
        &json!({ "queue": data.queue, "active": data.active }),
    );
    sse::publish(&state.events, "progress", &json!(data.progress));
    sse::publish(&state.events, "history", &json!(data.history));
        (data.history.clone(), data.history_path.clone())
    };
    persist_history(&history_snapshot, &path);
}

fn classify_bridge_error(err: &BridgeError) -> ErrorClass {
    match err {
        BridgeError::Timeout(_) => ErrorClass::Transient,
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
        | DownloadError::InsufficientDiskSpace(_) => ErrorClass::Permanent,
        DownloadError::Client(_) | DownloadError::Http(_) => {
            retry::classify_error(&err.message(), None)
        }
    }
}

fn decide_retry(
    state: &AppState,
    url: &str,
    name: &str,
    platform: &str,
    current_task_id: &str,
    err_msg: &str,
    err_class: ErrorClass,
) -> RetryDecision {
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
        if let Some(e) = data
            .history
            .iter_mut()
            .find(|e| e.get("task_id").and_then(Value::as_str) == Some(current_task_id))
        {
            e["status"] = json!("Téléchargement");
            e["entity_state"] = json!("RETRY_SCHEDULED");
            e["retry_count"] = json!(new_failures);
            e["max_retries"] = json!(retry::DEFAULT_MAX_RETRIES);
            e["retry_at"] = json!(retry_at);
            e["message"] = json!(retry_message(name, new_failures, delay));
        }
        let new_task_id = retry_task_id(url);
        drop(data);
        push_queued_history_entry(
            state,
            &new_task_id,
            url,
            name,
            platform,
            "Queued",
            &retry_message(name, new_failures, delay),
            new_failures,
        );
        RetryDecision::Retry {
            new_task_id,
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
    let dest = dest_path_for(&downloads, &game_url, &game_name);

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
    {
        let mut d = c_state.write();
        d.retry_in_flight.insert(c_url.clone());
    }
    let cancel: Arc<Notify> = {
        let mut d = c_state.write();
        let sig = Arc::new(Notify::new());
        d.cancel_signals.insert(c_url.clone(), sig.clone());
        sig
    };
    let shutdown = c_state.shutdown.clone();
    tokio::spawn(async move {
        // Gap-4 4a — bellek içi `bytes()` yerine `HttpDownloader` stream motoru
        // (`.part` yazma, Range resume, challenge/HTML/arşiv guards, cancel).
        // TASK-002-gap-1: job-level retry envelope (torrent ile ortak).
        let mut current_task_id = c_task.clone();
        let current_url = c_url.clone();
        let mut aborted: Option<String> = None;
        loop {
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
            let result = manager_download::http::HttpDownloader::new()
                .with_cancel(cancel_flag)
                .with_retry(1, Duration::from_secs(5))
                .with_progress(move |downloaded, total| {
                    let pct = if total > 0 {
                        (downloaded * 100 / total) as u32
                    } else {
                        0
                    };
                    let mut data = progress_state.write();
                    data.progress[&progress_url] =
                        json!({ "status": "Downloading", "progress": pct });
                    sse::publish(
                        &progress_state.events,
                        "progress",
                        &json!(data.progress),
                    );
                })
                .download_async(&req)
                .await;

            match result {
                Ok(path) => {
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
                    let cls = classify_download_error(&e);
                    match decide_retry(
                        &c_state,
                        &current_url,
                        &c_name,
                        &c_plat,
                        &current_task_id,
                        &e.message(),
                        cls,
                    ) {
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

/// Bilinen ROM / torrent dosya uzantısı (Python `check_extension_before_download`).
fn known_torrent_extension(seg: &str) -> bool {
    match std::path::Path::new(seg).extension().and_then(|e| e.to_str()) {
        Some(ext) => {
            let ext = ext.to_ascii_lowercase();
            matches!(
                ext.as_str(),
                "torrent" | "zip" | "7z" | "rar" | "iso" | "chd" | "cue" | "bin"
                    | "gdi" | "nes" | "snes" | "smc" | "gb" | "gbc" | "gba" | "nds"
                    | "n64" | "z64" | "v64" | "psp" | "pbp" | "cso" | "img" | "ccd"
                    | "m3u" | "sv" | "wbfs" | "wad" | "xci" | "nsp"
            )
        }
        None => false,
    }
}

/// Dosya adı olarak kullanılacak metni temizler (path ayracı yasak).
fn sanitize_file_name(name: &str) -> String {
    name.replace(['/', '\\', ':'], "_")
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
            &downloading, &queue_ids, &queue_urls, &retry_urls, &prog
        ));

        let queued_active = json!({ "status": "Queued", "task_id": "t1", "url": "http://x" });
        assert!(is_active_history_entry(
            &queued_active, &queue_ids, &queue_urls, &retry_urls, &prog
        ));

        // Queued ama ne kuyrukta ne de aktif → korunmaz.
        let queued_orphan = json!({ "status": "Queued", "task_id": "nope", "url": "http://orphan" });
        assert!(!is_active_history_entry(
            &queued_orphan, &queue_ids, &queue_urls, &retry_urls, &prog
        ));

        // Tamamlanmış → korunmaz.
        let done = json!({ "status": "Download_OK", "task_id": "t1", "url": "http://x" });
        assert!(!is_active_history_entry(
            &done, &queue_ids, &queue_urls, &retry_urls, &prog
        ));

        // Seeding her zaman korunur.
        let seeding = json!({ "status": "Seeding", "task_id": "x", "url": "http://y" });
        assert!(is_active_history_entry(
            &seeding, &queue_ids, &queue_urls, &retry_urls, &prog
        ));
    }
}