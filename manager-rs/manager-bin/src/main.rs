//! manager-bin: boş state ile HTTP+SSE sunucusu (smoke).
//!
//! TASK-002b doğrulama: `manager-http` router'ı ayakta; gerçek iş mantığı
//! (queue worker, tray, autostart) TASK-002c/002d'de bağlanır.
//!
//! Varsayılan port 5010 (çakışma riski için 5000 değil — canlı manager'dan
//! ayrı durur); `RGSX_MANAGER_BIN_PORT` env ile değiştirilebilir.

use manager_core::state::ManagerState;
use manager_http::{router, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        )
        .init();

    let port: u16 = std::env::var("RGSX_MANAGER_BIN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5010);

    let mut data = manager_http::StateData::empty();
    data.manager_state = ManagerState::Running;
    let app = router(AppState::with_data(data, manager_http::sse::channel()));

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("manager-bin smoke listening on http://{addr}");
    tracing::info!("GET /api/health, /api/queue, /api/events (SSE)");

    axum::serve(listener, app).await.unwrap();
}