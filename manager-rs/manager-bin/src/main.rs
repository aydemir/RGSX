//! manager-bin: Python bridge + HTTP+SSE sunucusu.
//!
//! TASK-002c: `qbittorrent_backend.py --bridge` subprocess'i başlatılır ve
//! `AppState`'e verilir; qbittorrent `/api/*` handler'ları gerçek Python
//! mantığına proxy eder. Queue worker/tray/autostart TASK-002d'de bağlanır.
//!
//! Varsayılan port 5010 (çakışma riski için 5000 değil — canlı manager'dan
//! ayrı durur); `RGSX_MANAGER_BIN_PORT` env ile değiştirilebilir.
//!
//! Bridge script yolu: `RGSX_MANAGER_SCRIPT` env, yoksa varsayılan
//! `../ports/RGSX/qbittorrent_backend.py` (workspace kökünden göreli).

use std::sync::Arc;

use manager_bridge::{Bridge, BridgeConfig};
use manager_core::state::ManagerState;
use manager_http::{router, AppState, StateData};

fn resolve_script() -> String {
    if let Ok(p) = std::env::var("RGSX_MANAGER_SCRIPT") {
        return p;
    }
    // cargo run workdir = workspace kökü (manager-rs). Python tarafı bir üstte.
    let fallback = std::path::Path::new("..")
        .join("ports")
        .join("RGSX")
        .join("qbittorrent_backend.py");
    fallback
        .canonicalize()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| fallback.to_string_lossy().to_string())
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let port: u16 = std::env::var("RGSX_MANAGER_BIN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5010);

    // Python bridge'i başlat (script yoksa None — placeholder davranışı).
    let script = resolve_script();
    let bridge = Bridge::spawn(BridgeConfig {
        script: script.clone(),
        timeout_secs: 90,
        ..BridgeConfig::default()
    });
    let bridge = match bridge {
        Ok(b) => {
            tracing::info!("bridge başlatıldı: {script}");
            Some(Arc::new(b))
        }
        Err(e) => {
            tracing::warn!("bridge başlatılamadı (placeholder davranışı): {e}");
            None
        }
    };

    let mut data = StateData::empty();
    data.manager_state = ManagerState::Running;
    let app = router(AppState {
        data: Arc::new(std::sync::RwLock::new(data)),
        events: manager_http::sse::channel(),
        bridge,
    });

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("manager-bin listening on http://{addr}");
    tracing::info!("GET /api/health, /api/queue, /api/events (SSE), qbittorrent proxy");

    axum::serve(listener, app).await.unwrap();
}
