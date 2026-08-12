//! manager-bin: Python bridge + HTTP+SSE sunucusu.
//!
//! TASK-002c: `qbittorrent_backend.py --bridge` subprocess'i başlatılır ve
//! `AppState`'e verilir; qbittorrent `/api/*` handler'ları gerçek Python
//! mantığına proxy eder.
//!
//! TASK-002d: Windows'ta (`cfg(windows)`) sistem tepsisi + auto-start (registry)
//! + firewall kuralı bağlanır.
//!
//! Varsayılan port 5010 (çakışma riski için 5000 değil — canlı manager'dan
//! ayrı durur); `RGSX_MANAGER_BIN_PORT` env ile değiştirilebilir.
//!
//! Bridge script yolu: `RGSX_MANAGER_SCRIPT` env, yoksa varsayılan
//! `../ports/RGSX/qbittorrent_backend.py` (workspace kökünden göreli).

use std::sync::Arc;
use std::time::Duration;

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

/// İkon dosyasını bridge script'inin yanından bulur (`assets/images/favicon_rgsx.ico`).
fn resolve_icon(script: &str) -> Option<String> {
    let p = std::path::Path::new(script)
        .parent()?
        .join("assets")
        .join("images")
        .join("favicon_rgsx.ico");
    p.is_file().then(|| p.to_string_lossy().to_string())
}

/// Windows'ta tray + autostart + firewall kurulumu (başarısızlık kritik değildir).
#[cfg(windows)]
fn setup_windows(_port: u16, script: &str) -> Option<manager_windows::tray::Tray> {
    use manager_windows::autostart;
    use manager_windows::firewall;
    use manager_windows::tray::{Tray, TrayConfig};

    // Auto-start: Python davranışı — pref varsayılan AÇIK; ilk çalıştırmada kur.
    if !autostart::is_enabled() {
        match autostart::install(&autostart::command_self()) {
            Ok(()) => tracing::info!("auto-start kuruldu (HKCU Run)"),
            Err(e) => tracing::warn!("auto-start kurulamadı: {e}"),
        }
    } else {
        tracing::debug!("auto-start zaten kayıtlı");
    }

    // Firewall: yönetici yetkisi gerektirir; başarısızlık uyarı olarak yansır.
    if let Ok(exe) = std::env::current_exe() {
        match firewall::add_rule(&exe) {
            Ok(()) => tracing::info!("firewall kuralı eklendi: RGSX Manager"),
            Err(e) => tracing::warn!("firewall kuralı eklenemedi (admin gerekebilir): {e}"),
        }
    }

    // Tray: ikon yoksa placeholder ile yine de başlat.
    let icon_path = resolve_icon(script).unwrap_or_default();
    let checked = autostart::is_enabled();
    match Tray::start(TrayConfig { icon_path, autostart_checked: checked }) {
        Ok(t) => {
            tracing::info!("tray ikonu başlatıldı");
            Some(t)
        }
        Err(e) => {
            tracing::warn!("tray başlatılamadı: {e}");
            None
        }
    }
}

#[cfg(not(windows))]
fn setup_windows(_port: u16, _script: &str) -> Option<()> {
    None
}

#[cfg(windows)]
use manager_windows::tray as manager_windows_tray;
#[cfg(not(windows))]
mod manager_windows_tray {
    pub struct Tray;
    #[derive(Debug)]
    pub enum TrayAction {
        OpenUi,
        OpenSettings,
        OpenDownloads,
        OpenLogs,
        Quit,
    }
}

/// URL'yi varsayılan tarayıcıda açar (cmd /c start — Windows; diğerinde no-op).
fn open_url(url: &str) {
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
}

/// Klasörü Explorer'da açar (`explorer <path>` — yol args olarak güvenlidir).
fn open_folder(path: &str) {
    if path.is_empty() {
        return;
    }
    let _ = std::process::Command::new("explorer")
        .arg(path)
        .spawn();
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
        bridge: bridge.clone(),
    });

    let addr = format!("127.0.0.1:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("manager-bin listening on http://{addr}");
    tracing::info!("GET /api/health, /api/queue, /api/events (SSE), qbittorrent proxy");

    // Windows: tray + autostart + firewall; eylemler ana döngüde beslenir.
    let tray = setup_windows(port, &script);

    if let Some(tray) = tray {
        run_with_tray(tray, port, bridge, app, listener).await;
    } else {
        axum::serve(listener, app).await.unwrap();
    }
}

/// Tray eylemlerini işlerken sunucuyu ayakta tutar.
async fn run_with_tray(
    tray: manager_windows_tray::Tray,
    port: u16,
    bridge: Option<Arc<Bridge>>,
    app: axum::Router,
    listener: tokio::net::TcpListener,
) {
    let base = format!("http://localhost:{port}");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Tray eylemlerini izleyen task; Quit gelince shutdown sinyali verir.
    let tray_task = tokio::spawn(async move {
        loop {
            match tray.try_action() {
                Ok(action) => {
                    use manager_windows_tray::TrayAction;
                    match action {
                        TrayAction::OpenUi => open_url(&format!("{base}/")),
                        TrayAction::OpenSettings => open_url(&format!("{base}/settings")),
                        TrayAction::OpenDownloads | TrayAction::OpenLogs => {
                            // Klasör yolunu bridge'den al; yoksa uyarı düş.
                            let is_downloads = action == TrayAction::OpenDownloads;
                            let bridge = bridge.as_ref();
                            let paths = match bridge {
                                Some(b) => b.get_app_paths().await.ok(),
                                None => None,
                            };
                            match paths {
                                Some((downloads, logs)) => {
                                    let target = if is_downloads { &downloads } else { &logs };
                                    if target.is_empty() {
                                        tracing::warn!("klasör yolu boş: {action:?}");
                                    } else {
                                        tracing::info!("Explorer açılıyor: {target}");
                                        open_folder(target);
                                    }
                                }
                                None => tracing::warn!("klasör yolu alınamadı (bridge): {action:?}"),
                            }
                        }
                        TrayAction::Quit => {
                            tracing::info!("tray Exit — kapanıyor");
                            let _ = shutdown_tx.send(());
                            break;
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Sunucuyu çalıştır; tray Quit → graceful shutdown.
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        })
        .await
        .unwrap();

    let _ = tray_task.abort();
}
