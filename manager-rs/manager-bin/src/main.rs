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

use tokio::sync::Notify;

use manager_bridge::{Bridge, BridgeConfig, TorrentBackend};
use manager_core::state::ManagerState;
use manager_http::{router, AppState, StateData};

mod paths;

fn resolve_script() -> String {
    // gap-26: RGSX_MANAGER_SCRIPT artık resolve_paths() tarafından (exe'den türetilmiş)
    // set edilir. Fallback: exe'yi içeren rgsx_dir'deki qbittorrent_backend.py (CWD-göreli DEĞİL).
    if let Ok(p) = std::env::var("RGSX_MANAGER_SCRIPT") {
        return p;
    }
    let dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let fallback = dir.join("qbittorrent_backend.py");
    fallback.to_string_lossy().to_string()
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

fn main() {
    // gap-26: tracing İLK (path-resolution logları görünsün), ardından single-thread
    // resolve_paths() + tokio runtime. std::env::set_var thread-safe DEĞİL (Rust 1.80+
    // unsafe) — run() öncesi tek thread'de güvenli.
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    // gap-26 ZORUNLU SIRA: path-resolution TEK thread'de, tokio runtime BAŞLAMADAN ÖNCE.
    let paths = paths::resolve_paths();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(run(paths));
}

async fn run(paths: paths::RgsxPaths) {

    let port: u16 = {
        // gap-27: saf-Rust varsayılan = true (port 5000). Flag yine env ile override edilebilir.
        let rust_webui = std::env::var("RGSX_RUST_WEBUI").map(|v| v == "1").unwrap_or(true);
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
    // TASK-002-gap-10 (A): history.json diske kalıcılaştır. Varsayılan yol
    // `RGSX_DATA_DIR/history.json` (paths.rs `RGSX_DATA_DIR`'i türetir); `RGSX_HISTORY_PATH`
    // set ise ona öncelik ver. Startup'ta mevcut history'yi yükle (geçersiz entry filtreli).
    let history_path = std::env::var("RGSX_HISTORY_PATH")
        .ok()
        .filter(|s| !s.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var("RGSX_DATA_DIR")
                .ok()
                .filter(|s| !s.is_empty())
                .map(|d| std::path::PathBuf::from(d).join("history.json"))
        });
    if let Some(ref hp) = history_path {
        data.history = manager_http::persist::load_history(hp);
        tracing::info!("history yüklendi: {} entry ({}", data.history.len(), hp.display());
    }
    data.history_path = history_path;
    // WebUI statik kökü: gap-26 — resolve_paths()'in türettinceği `webui_dir` doğrudan
    // kullanılır. `RGSX_WEBUI_DIR` env'den tekrar okumak set_var→var zincirinde hata
    // vermişti (placeholder "<h1>RGSX Manager" servisi). env_or() override (RGSX_WEBUI_DIR
    // set ise) paths.rs'de hâlâ geçerlidir; fakat static_root env'ye bağlanmaz.
    let static_root = Some(paths.webui_dir.clone());
    // Faz 12c — native catalog: `RGSX_NATIVE_CATALOG=1` ise Python'sız local
    // dosyalardan üretir (systems_list.json, games/, languages/, images/). Komut
    // POST'ları için yine de Python'a proxy edebilir (`RGSX_PYTHON_MANAGER_URL`).
    // Aksi halde `RGSX_PYTHON_MANAGER_URL` set ise Python proxy (Faz 10c/3/2).
    // gap-27: saf-Rust varsayılan = true (native catalog). Flag yine env ile override edilebilir.
    let native_catalog = std::env::var("RGSX_NATIVE_CATALOG")
        .map(|v| v == "1")
        .unwrap_or(true);
    let catalog: Option<Arc<dyn manager_http::catalog::CatalogSource>> = if native_catalog {
        // Faz 12f: native katalog verisi (systems_list.json + games/) eksikse OTA'dan çek.
        manager_http::catalog_bootstrap::ensure_catalog_ready().await;
        Some(Arc::new(manager_http::catalog::NativeCatalog::from_env()))
    } else {
        std::env::var("RGSX_PYTHON_MANAGER_URL")
            .ok()
            .filter(|u| !u.is_empty())
            .map(|base| Arc::new(manager_http::catalog::PythonCatalog::new(base)) as Arc<dyn manager_http::catalog::CatalogSource>)
    };

    // Faz 12.6a — startup'ta diskteki kurulu oyunları tara; snapshot `downloaded`
    // (İndirilenler sekmesi) bununla dolar. `/api/game-status` ise isteğe bağlı
    // canlı tarama yapar (aynı mantık, NativeCatalog.installed_list üzerinden).
    if let Some(c) = &catalog {
        let installed = c.installed_list();
        data.downloaded = serde_json::json!(installed);
        // F3-F4: O(1) "already downloaded?" indeksini kurulumdan türet.
        data.rebuild_downloaded_index();
        let total: usize = installed.values().map(|v| v.len()).sum();
        tracing::info!(
            "disk taraması: {} platformda {} kurulu oyun bulundu",
            installed.len(),
            total
        );
    }

    // Faz 12.6d — eşzamanlı indirme sınırı semaphore kapasitesi (ayar'dan türet).
    let max_dl = manager_core::settings::Settings::load()
        .max_simultaneous_downloads
        .max(1) as usize;
    data.download_semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_dl));
    data.max_simultaneous_downloads = max_dl;

    let events = manager_http::sse::channel();
    // TASK-002-gap-1 (BELİRSİZ-2): global shutdown sinyali — retry döngülerinin
    // `tokio::select!` ile dinlediği `AppState.shutdown` Notify'ı. Aynı Arc hem
    // `AppState`'e hem tray Quit handler'ına geçer (iki ayrı instance OLMAZ).
    let shutdown = Arc::new(Notify::new());
    let app = router({
        let (tx, rx) = tokio::sync::mpsc::channel::<manager_http::state::QueueCommand>(1024);
        let state = AppState {
            data: Arc::new(std::sync::RwLock::new(data)),
            events: events.clone(),
            bridge: bridge.clone(),
            static_root: static_root.clone(),
            catalog: catalog.clone(),
            shutdown: shutdown.clone(),
            tx,
            global_paused: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            dirty: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        };
        tokio::spawn(manager_http::api::queue_worker(rx, state.clone()));
        state
    });

    // TASK-005-B — native SDL2/gilrs gamepad girdi yolu. `native-input` feature
    // derlenmişse VE `RGSX_NATIVE_INPUT=1` set ise, bağlı gamepad'i ES map ile
    // okuyup SSE `gamepad` olayı olarak yukarıdaki kanaldan yayar (webui TV
    // modu tüketir). Headless/sandbox'ta gilrs başlatılamazsa sessizce atlanır.
    #[cfg(feature = "native-input")]
    if std::env::var("RGSX_NATIVE_INPUT").map(|v| v == "1").unwrap_or(false) {
        let es = manager_http::es_input::load_best();
        manager_tvui::native_input::start_native_input(events.clone(), es);
    }

    // Faz 12b — TVUI shell: `RGSX_TVUI=1` ise SPA'yı kiosk/webview'da açar.
    // Ayrı thread'de (webview feature event loop'u bloklar); kiosk yolunda
    // tarayıcıyı spawn edip döner. Headless ortamda hata loglanır, sunucu etkilenmez.
    if std::env::var("RGSX_TVUI").map(|v| v == "1").unwrap_or(false) {
        let tv_port = port;
        std::thread::spawn(move || {
            if let Err(e) = manager_tvui::launch(tv_port) {
                tracing::warn!("TVUI başlatılamadı: {e}");
            }
        });
    }

    // Faz 12.6d — tüm arayüzlere bağlan (0.0.0.0) ki konteyner IP'si
    // (örn. 192.168.1.6) üzerinden uzak tarayıcıdan erişilebilsin.
    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    tracing::info!("manager-bin listening on http://{addr}");
    // LAN IP'sini UDP-socket numarasıyla bul (veri göndermeden) — uzak
    // tarayıcı erişimi için gerçek adresi de logla (Python server.py ile parite).
    match local_lan_ip() {
        Some(ip) => tracing::info!("Ağ erişimi: http://{ip}:{port}"),
        None => tracing::warn!("LAN IP belirlenemedi; yalnız 0.0.0.0:{port} görünür"),
    }
    tracing::info!("GET /api/health, /api/queue, /api/events (SSE), qbittorrent proxy");

    // Windows: tray + autostart + firewall; eylemler ana döngüde beslenir.
    let tray = setup_windows(port, &script);

    if let Some(tray) = tray {
        run_with_tray(tray, port, bridge, app, listener, shutdown.clone()).await;
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
    shutdown: Arc<Notify>,
) {
    let base = format!("http://localhost:{port}");
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

    // Tray eylemlerini izleyen task; Quit gelince shutdown sinyali verir.
    // `shutdown_notify` = AppState.shutdown ile AYNI Arc (retry döngülerini keser).
    let shutdown_notify = shutdown.clone();
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
                            // İki bağımsız sinyal: `shutdown_tx` axum'ın graceful
                            // shutdown'ı; `shutdown_notify` aktif retry döngülerini keser.
                            shutdown_notify.notify_one();
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

/// UDP-socket numarasıyla gerçek LAN IP'sini bulur (veri göndermeden).
/// Başarısızsa None döner. Platform bağımsızdır (std::net).
fn local_lan_ip() -> Option<String> {
    use std::net::UdpSocket;
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    sock.local_addr().ok().map(|a| a.ip().to_string())
}
