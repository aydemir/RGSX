//! manager-http: HTTP + SSE route'ları (tokio + axum).
//!
//! TASK-002b — Python `ManagerHandler`/`RGSXHandler` sözleşmesi 1:1:
//! `/api/*` GET/POST + `/api/events` (SSE). Yanıt şablonları
//! `tests/test_api_contract.py` ile birebir (bkz. `api.rs` docstring).
//!
//! Kullanım:
//! ```no_run
//! use manager_http::{router, AppState};
//! # async fn scaffold() {
//! let app = router(AppState::empty());
//! let listener = tokio::net::TcpListener::bind("127.0.0.1:5010").await.unwrap();
//! axum::serve(listener, app).await.unwrap();
//! # }
//! ```

pub mod api;
pub mod catalog;
pub mod catalog_bootstrap;
pub mod es_input;
pub mod persist;
pub mod sse;
pub mod self_update;
pub mod state;

use axum::routing::{get, post, get_service};
use axum::Router;
use tower_http::services::ServeDir;

pub use state::{AppState, StateData};

/// İş mantığı SaaS'ı — router + yanıt şablonları.
///
/// Route yolu 1:1 Python `do_GET`/`do_POST` dispatch'i; `/api/events` SSE.
/// Statik katman (Faz 12a): `static_root` varsa `/static/*` `ServeDir` ile sunulur,
/// `/` ve bilinmeyen SPA route'ları hydrate edilmiş `index.html` döndürür
/// (client-side routing). `static_root` yoksa statik kapalı (404 fallback).
pub fn router(app: AppState) -> Router {
    let api = Router::new()
        .route("/api/platforms", get(api::platforms))
        .route("/api/search", get(api::search))
        .route("/api/translations", get(api::translations))
        .route("/api/languages", get(api::languages))
        .route("/api/games/:platform", get(api::games))
        .route("/api/progress", get(api::progress))
        .route("/api/game-status", get(api::game_status))
        .route("/api/history", get(api::history))
        .route("/api/queue", get(api::queue).post(api::queue_post))
        .route("/api/settings", get(api::settings_get).post(api::settings_post))
        .route("/api/system_info", get(api::system_info))
        .route("/api/browse-directories", get(api::browse_directories))
        .route("/api/scan", get(api::scan))
        .route("/api/image/:platform", get(api::image))
        .route("/api/es-input", get(api::es_input))
        .route("/api/favicon", get(api::favicon))
        .route("/api/update-cache", get(api::update_cache))
        .route("/api/manager-update", get(api::manager_update_status))
        .route("/api/manager-update/download", post(api::manager_update_download))
        .route("/api/manager-update/apply", post(api::manager_update_apply))
        .route("/api/catalog/retry", post(api::catalog_retry))
        .route("/api/download", post(api::download))
        .route("/api/download/batch", post(api::download_batch))
        .route("/api/cancel", post(api::cancel))
        .route("/api/queue/clear", post(api::queue_clear))
        .route("/api/queue/remove", post(api::queue_remove))
        .route("/api/save_filters", post(api::save_filters))
        .route("/api/clear-history", post(api::clear_history))
        .route("/api/restart", post(api::restart))
        .route("/api/support", post(api::support))
        .route("/api/health", get(api::health))
        .route("/api/shutdown", post(api::shutdown))
        .route("/api/pause", post(api::pause))
        .route("/api/resume", post(api::resume))
        .route("/api/qbittorrent/change-password", post(api::change_password))
        .route("/api/qbittorrent/start", post(api::qb_start))
        .route("/api/qbittorrent/password-status", get(api::qb_password_status))
        .route("/api/qbittorrent/regenerate-password", post(api::qb_regenerate_password))
        .route("/api/events", get(sse::events));

    // Statik katman (Faz 12a): SPA sunumu. `static_root` yoksa statik kapalı.
    let api = if let Some(root) = &app.static_root {
        api.route("/", get(api::index))
            .nest(
                "/static",
                Router::new().fallback(get_service(
                    ServeDir::new(root.clone()).append_index_html_on_directories(false),
                )),
            )
            .fallback(api::index)
    } else {
        api.route("/", get(api::index)).fallback(api::fallback)
    };

    api.with_state(app)
}