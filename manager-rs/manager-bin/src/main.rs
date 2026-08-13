//! manager-bin: torrent engine + HTTP+SSE sunucusu.
//!
//! TASK-002c (Python engine): `qbittorrent_backend.py --bridge` subprocess'i
//! başlatılır ve `AppState`'e verilir; qbittorrent `/api/*` handler'ları gerçek
//! Python mantığına proxy eder.
//!
//! TASK-002f (librqbit engine): `RGSX_TORRENT_ENGINE=librqbit` set edilirse
//! Python bridge yerine in-process librqbit engine (`manager-torrent`) kurulur;
//! aynı `TorrentBackend` sözleşmesini konuştuğundan handler'lar değişmez.
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

use manager_bridge::{Bridge, BridgeConfig, TorrentBackend};
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

/// Torrent engine'ini `RGSX_TORRENT_ENGINE` env'ine göre kurar.
///
/// - `python` → legacy Python bridge subprocess (qbittorrent_backend.py; WebUI/
///   port-fallback/şifre migration korunur). TASK-002f öncesi davranış — opt-in.
/// - `librqbit` / boş / diğer → **varsayılan**: in-process librqbit (manager-torrent).
///   TASK-002g ertelenmiş kararı (2026-08-12): librqbit Windows'ta da derlendiği
///   (`cargo check --target x86_64-pc-windows-gnu`) doğrulanınca varsayılan yapıldı.
fn resolve_engine() -> Option<Arc<dyn TorrentBackend>> {
    match std::env::var("RGSX_TORRENT_ENGINE").as_deref() {
        Ok("python") => {
            let script = resolve_script();
            match Bridge::spawn(BridgeConfig {
                script: script.clone(),
                timeout_secs: 90,
                ..BridgeConfig::default()
            }) {
                Ok(b) => {
                    tracing::info!("torrent engine: python bridge ({script})");
                    Some(Arc::new(b))
                }
                Err(e) => {
                    tracing::warn!("bridge başlatılamadı ({script}): {e}");
                    None
                }
            }
        }
        _ => {
            let downloads = std::env::var("RGSX_DOWNLOADS_FOLDER")
                .unwrap_or_else(|_| std::env::temp_dir().join("rgsx_torrents").to_string_lossy().to_string());
            let logs = std::env::var("RGSX_LOGS_FOLDER")
                .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
            let engine = manager_torrent::LibrqbitEngine::new(
                std::path::PathBuf::from(&downloads),
                downloads,
                logs,
            );
            tracing::info!("torrent engine: librqbit (embedded, varsayılan)");
            Some(Arc::new(engine))
        }
    }
}

/// İkon dosyasını bridge script'inin yanından bulur (`assets/images/favicon_rgsx.ico`).
#[cfg(windows)]
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
    // `RGSX_NO_AUTOSTART=1` (ör. .bat launcher sidecar): Python manager aynı
    // registry anahtarını (RGSXManager) yönettiğinden sidecar kayıt yapmaz.
    let no_autostart = std::env::var("RGSX_NO_AUTOSTART").map(|v| v == "1").unwrap_or(false);
    if !no_autostart && !autostart::is_enabled() {
        match autostart::install(&autostart::command_self()) {
            Ok(()) => tracing::info!("auto-start kuruldu (HKCU Run)"),
            Err(e) => tracing::warn!("auto-start kurulamadı: {e}"),
        }
    } else {
        tracing::debug!("auto-start kayıtlı değil/atlandı (RGSX_NO_AUTOSTART={no_autostart})");
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
fn setup_windows(_port: u16, _script: &str) -> Option<manager_windows_tray::Tray> {
    None
}

#[cfg(windows)]
use manager_windows::tray as manager_windows_tray;
#[cfg(not(windows))]
#[allow(dead_code)]
mod manager_windows_tray {
    use std::sync::mpsc::TryRecvError;

    /// Windows dışı stub — tray yok; arayüz Windows karşılığıyla aynı kalır.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum TrayAction {
        OpenUi,
        OpenSettings,
        OpenDownloads,
        OpenLogs,
        Quit,
    }

    pub struct Tray;

    impl Tray {
        /// Asla eylem üretmez; tray olmadığı için kanal hep "koptu" sayılır.
        pub fn try_action(&self) -> Result<TrayAction, TryRecvError> {
            Err(TryRecvError::Disconnected)
        }
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

    let port: u16 = {
        let rust_webui = std::env::var("RGSX_RUST_WEBUI").map(|v| v == "1").unwrap_or(false);
        let default = if rust_webui { 5000 } else { 5010 };
        std::env::var("RGSX_MANAGER_BIN_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(default)
    };

    // Python bridge'i başlat (script yoksa None — placeholder davranışı).
    let script = resolve_script();
    let bridge = resolve_engine();

    let mut data = StateData::empty();
    data.manager_state = ManagerState::Running;
    // WebUI statik kökü: `RGSX_WEBUI_DIR` set ise onu kullan, yoksa bridge
    // script'inin yanındaki `static/` klasörü (varsa).
    let static_root = std::env::var("RGSX_WEBUI_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::path::Path::new(&script)
                .parent()
                .map(|p| p.join("static"))
                .filter(|p| p.is_dir())
        });
    // Faz 10c/3/2: katalog proxy kaynağı — `RGSX_PYTHON_MANAGER_URL` set ise Python'a
    // bağlanır (devre dışıysa handler'lar placeholder'a düşer, geriye uyumlu).
    let catalog = std::env::var("RGSX_PYTHON_MANAGER_URL")
        .ok()
        .filter(|u| !u.is_empty())
        .map(|base| Arc::new(manager_http::catalog::PythonCatalog::new(base)) as Arc<dyn manager_http::catalog::CatalogSource>);

    let app = router(AppState {
        data: Arc::new(std::sync::RwLock::new(data)),
        events: manager_http::sse::channel(),
        bridge: bridge.clone(),
        static_root,
        catalog,
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
    bridge: Option<Arc<dyn TorrentBackend>>,
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
