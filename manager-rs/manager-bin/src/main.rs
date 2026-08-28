//! manager-bin: torrent engine + HTTP+SSE sunucusu.
//!
//! TASK-013 (qBittorrent emekliliği): tek torrent yolu = in-process librqbit
//! (`manager-torrent`, `TorrentBackend` sözleşmesi). Eski `RGSX_TORRENT_ENGINE=python`
//! subprocess yolu + `/api/qbittorrent/*` uçları söküldü (Python port donuk
//! referansta yaşamaya devam eder).
//!
//! TASK-002d: Windows'ta (`cfg(windows)`) sistem tepsisi + auto-start (registry)
//! + firewall kuralı bağlanır.
//!
//! Varsayılan port 5000 (`RGSX_RUST_WEBUI=0` ile 5010); `RGSX_MANAGER_BIN_PORT`
//! env ile değiştirilebilir.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

use manager_bridge::TorrentBackend;
use manager_core::state::ManagerState;
use manager_http::{router, AppState, StateData};

mod paths;

/// Torrent engine'ini kurar — TASK-013: **librqbit tek yol**. Eski
/// `RGSX_TORRENT_ENGINE` env'i (python/librqbit seçimi) emekli edildi;
/// varsa yok sayılır.
fn resolve_engine() -> Option<Arc<dyn TorrentBackend>> {
    let downloads = std::env::var("RGSX_DOWNLOADS_FOLDER").unwrap_or_else(|_| {
        std::env::temp_dir()
            .join("rgsx_torrents")
            .to_string_lossy()
            .to_string()
    });
    let logs = std::env::var("RGSX_LOGS_FOLDER")
        .unwrap_or_else(|_| std::env::temp_dir().to_string_lossy().to_string());
    let engine =
        manager_torrent::LibrqbitEngine::new(std::path::PathBuf::from(&downloads), downloads, logs);
    tracing::info!("torrent engine: librqbit (embedded, tek yol)");
    Some(Arc::new(engine))
}

/// İkon dosyasını exe'nin yanından bulur (`assets/images/favicon_rgsx.ico`).
/// gap-02: eski anchor Python script diziniydi; native-only'de ikon exe ile
/// aynı dizinde beklenir (yoksa tray placeholder kullanır).
#[cfg(windows)]
fn resolve_icon() -> Option<String> {
    let dir = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(|p| p.to_path_buf()))?;
    let p = dir.join("assets").join("images").join("favicon_rgsx.ico");
    p.is_file().then(|| p.to_string_lossy().to_string())
}

/// Windows'ta tray + autostart + firewall kurulumu (başarısızlık kritik değildir).
#[cfg(windows)]
fn setup_windows(_port: u16) -> Option<manager_windows::tray::Tray> {
    use manager_windows::autostart;
    use manager_windows::firewall;
    use manager_windows::tray::{Tray, TrayConfig};

    // Auto-start: Python davranışı — pref varsayılan AÇIK; ilk çalıştırmada kur.
    // `RGSX_NO_AUTOSTART=1` (ör. .bat launcher sidecar): Python manager aynı
    // registry anahtarını (RGSXManager) yönettiğinden sidecar kayıt yapmaz.
    let no_autostart = std::env::var("RGSX_NO_AUTOSTART")
        .map(|v| v == "1")
        .unwrap_or(false);
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
    let icon_path = resolve_icon().unwrap_or_default();
    let checked = autostart::is_enabled();
    match Tray::start(TrayConfig {
        icon_path,
        autostart_checked: checked,
    }) {
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
fn setup_windows(_port: u16) -> Option<manager_windows_tray::Tray> {
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
    let _ = std::process::Command::new("explorer").arg(path).spawn();
}

/// TASK-012l cutover: `RGSX_TVUI=1` varsayılan, `0`/false ile kapatılabilir.
pub fn is_tvui_enabled() -> bool {
    match std::env::var("RGSX_TVUI") {
        Ok(v) => {
            let s = v.trim().to_ascii_lowercase();
            !(s == "0" || s == "false" || s == "off" || s.is_empty())
        }
        Err(_) => true, // varsayılan 1
    }
}

fn main() {
    // TASK-012m Faz 5 — `--recover`: önceki apply'ın .old yedeğinden geri yükle
    // (rollback). Sunucu BAŞLAMADAN tek-seferlik çalışır ve çıkar.
    if std::env::args().any(|a| a == "--recover") {
        match manager_http::self_update::recover_update(None) {
            Ok(()) => {
                eprintln!("recover: .old yedeğinden geri yüklendi");
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("recover hatası: {e}");
                std::process::exit(1);
            }
        }
    }

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
        let rust_webui = std::env::var("RGSX_RUST_WEBUI")
            .map(|v| v == "1")
            .unwrap_or(true);
        let default = if rust_webui { 5000 } else { 5010 };
        std::env::var("RGSX_MANAGER_BIN_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(default)
    };

    // Torrent engine (TASK-013: librqbit tek yol).
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
        tracing::info!(
            "history yüklendi: {} entry ({}",
            data.history.len(),
            hp.display()
        );
    }
    data.history_path = history_path;
    // WebUI statik kökü: gap-26 — resolve_paths()'in türettinceği `webui_dir` doğrudan
    // kullanılır. `RGSX_WEBUI_DIR` env'den tekrar okumak set_var→var zincirinde hata
    // vermişti (placeholder "<h1>RGSX Manager" servisi). env_or() override (RGSX_WEBUI_DIR
    // set ise) paths.rs'de hâlâ geçerlidir; fakat static_root env'ye bağlanmaz.
    let static_root = Some(paths.webui_dir.clone());
    // Faz 12c — native catalog: local dosyalardan üretir (systems_list.json,
    // games/, languages/, images/). TASK-012-gap-02: Python proxy yolu söküldü —
    // tek katalog kaynağı NativeCatalog.
    // SSE kanalı erken kurulur ki katalog bootstrap'i (arka plan) ilerlemeyi yayabilsin.
    let events = manager_http::sse::channel();
    // Faz 12f: native katalog verisi (systems_list.json + games/) eksikse OTA'dan çek.
    // Faz 2b: bootstrap arka plana alındı; spawn manager-bin router closure içinde,
    // `state` (StateData) hazırken yapılır ki bootstrap bitince `catalog_ready` atomiği yazılabilsin.
    // Sunucu anında başlar, TVUI/WebUI `catalog_update` SSE'iyle loading bar'ını doldurur.
    // İlk çalıştırmada dosyalar henüz inmediğinden NativeCatalog boş açılır (`from_env` yalnız
    // yolu saklar, paniklemez); `load_sources()` her çağrıda diskten okuduğundan ready sonrası doluluk.
    let catalog: Option<Arc<dyn manager_http::catalog::CatalogSource>> =
        Some(Arc::new(manager_http::catalog::NativeCatalog::from_env()));

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
        // F6: 250ms throttled SSE (queue/history/progress/downloaded) yayını —
        // `AppState` burada constructor (empty/with_data) yerine struct literal
        // ile kurulduğu için broadcast_loop'u ayrıca spawn etmek gerekir;
        // aksi halde UI canlı güncellenmez (yalnız F5/REST state görünür).
        tokio::spawn(manager_http::sse::broadcast_loop(state.clone()));
        // Faz 2c-race: bootstrap tamamlanınca `StateData.catalog_ready` yazar; TVUI snapshot'tan
        // hazır olduğunu anlar (geç SSE abonesi loading bar'da sonsuza dek takılmaz).
        let boot_ev = events.clone();
        let boot_data = state.data.clone();
        tokio::spawn(async move {
            manager_http::catalog_bootstrap::ensure_catalog_ready(Some(&boot_ev), Some(boot_data))
                .await;
        });
        // TASK-012m: manager self-update arka plan kontrolü (RGSX_UPDATE_MANIFEST_URL
        // yapılandırılmışsa; aksi halde no-op). Yeniyse SSE `manager_update` + StateData.
        let upd_ev = events.clone();
        let upd_data = state.data.clone();
        tokio::spawn(async move {
            manager_http::self_update::check_update(upd_ev, upd_data).await;
        });
        state
    });

    // TASK-005-B — native SDL2/gilrs gamepad girdi yolu. `native-input` feature
    // derlenmişse VE `RGSX_NATIVE_INPUT=1` set ise, bağlı gamepad'i ES map ile
    // okuyup SSE `gamepad` olayı olarak yukarıdaki kanaldan yayar (webui TV
    // modu tüketir). Headless/sandbox'ta gilrs başlatılamazsa sessizce atlanır.
    #[cfg(feature = "native-input")]
    if std::env::var("RGSX_NATIVE_INPUT")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        let es = manager_http::es_input::load_best();
        manager_tvui::native_input::start_native_input(events.clone(), es);
    }

    // TVUI shell: `RGSX_TVUI` varsayılan 1 (TASK-012l cutover). `0` ile kapatılabilir.
    // Ayrı thread'de (SDL event loop'u bloklar). TASK-012-gap-03 Faz B: SPA
    // `?mode=tv` yolu emekli edildi — tek TVUI = native SDL2. Headless ortamda
    // hata loglanır, sunucu etkilenmez. Python pygame TVUI TASK-012-gap-02 ile söküldü.
    if is_tvui_enabled() {
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
    tracing::info!("GET /api/health, /api/queue, /api/events (SSE)");

    // Windows: tray + autostart + firewall; eylemler ana döngüde beslenir.
    let tray = setup_windows(port);

    if let Some(tray) = tray {
        run_with_tray(tray, port, bridge, app, listener, shutdown.clone()).await;
    } else {
        // Linux (tray yok): SIGTERM/SIGINT (Ctrl+C) VEYA AppState.shutdown
        // (POST /api/shutdown) → graceful shutdown + bridge/session temizliği.
        // Önceden `axum::serve(...).await` idi; sinyal yoktu → process "ölümsüz"
        // kalıyordu (kill -9 gerekliydi).
        axum::serve(listener, app)
            .with_graceful_shutdown(graceful_shutdown_signal(shutdown.clone(), bridge.clone()))
            .await
            .unwrap();
    }
}

/// Ortak graceful shutdown koordinatörü: OS sinyali (SIGTERM/SIGINT/Ctrl+C) VEYA
/// `AppState.shutdown` Notify'i tetiklenince tüm download/retry waiter'larını
/// uyandırır (notify_waiters) ve bridge/session + subprocess temizliğini yapar.
///
/// Hem tray'lı (Windows) hem traysız (Linux) yolda `axum::serve().with_graceful_shutdown`
/// için kullanılır; tek noktadan kapanış guarantee'si sağlar.
async fn graceful_shutdown_signal(shutdown: Arc<Notify>, bridge: Option<Arc<dyn TorrentBackend>>) {
    // OS sinyali VEYA AppState.shutdown Notify'i → kapanışı tetikle.
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate()).expect("SIGTERM dinlenemedi");
        let mut sigint = signal(SignalKind::interrupt()).expect("SIGINT dinlenemedi");
        tokio::select! {
            _ = sigterm.recv() => {}
            _ = sigint.recv() => {}
            _ = shutdown.notified() => {}
        }
    }
    #[cfg(windows)]
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = shutdown.notified() => {}
        }
    }
    // Çoklu download/retry waiter'ı + (yeniden) axum graceful shutdown waiter'ı
    // uyansın. `notify_one` yalnız BİR waiter'ı uyandırırdı → yapışkan process.
    shutdown.notify_waiters();
    // Engine kapanışı (librqbit session.stop). TASK-013: Python subprocess yolu emekli.
    if let Some(b) = bridge {
        let _ = b.call("shutdown", serde_json::json!({})).await;
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
    // Tray task'ı `bridge`'i move eder; graceful_shutdown_signal için ayrı clone.
    let bridge_for_tray = bridge.clone();

    // Tray eylemlerini izleyen task; Quit gelince shutdown sinyali verir.
    // `shutdown_notify` = AppState.shutdown ile AYNI Arc (retry döngülerini keser
    // ve `graceful_shutdown_signal`'ı uyandırır). Bridge temizliği tek noktadan
    // `graceful_shutdown_signal` içinde yapılır (çift çağrı önlenir).
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
                            let bridge = bridge_for_tray.as_ref();
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
                                None => {
                                    tracing::warn!("klasör yolu alınamadı (bridge): {action:?}")
                                }
                            }
                        }
                        TrayAction::Quit => {
                            tracing::info!("tray Exit — graceful shutdown");
                            // Tüm waiter'lar (retry döngüleri + axum graceful
                            // shutdown) uyanır; bridge/session temizliği
                            // `graceful_shutdown_signal` içinde yapılır.
                            shutdown_notify.notify_waiters();
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

    // Sunucuyu çalıştır; tray Quit (notify_waiters) VEYA OS sinyali → graceful shutdown.
    axum::serve(listener, app)
        .with_graceful_shutdown(graceful_shutdown_signal(shutdown.clone(), bridge.clone()))
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
