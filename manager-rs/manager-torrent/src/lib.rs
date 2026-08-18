//! librqbit embedded torrent engine — Python `qbittorrent_backend.py` ikamesi (Faz 10b).
//!
//! `LibrqbitEngine`, `manager_bridge::TorrentBackend` sözleşmesini implement eder;
//! Python bridge'in `_BRIDGE_METHODS` metod isimlerini ve yanıt şekillerini birebir
//! taklit eder, arka planda ise embedded librqbit `Session`'ı kullanır (WebUI yok).
//!
//! - `ping` → `"pong"`
//! - `status` → `{state: "STOPPED"|"RUNNING", available: true}`
//! - `is_available` → `true`
//! - `ensure_running` → Python session'ı lazy spawn eder, başarı = `true`
//! - `get_webui_url` → `""` (librqbit'te WebUI yoktur)
//! - `get_password_status` → `{available, using_default, secured, mode, webui_url}`
//! - `change_webui_password` → `(false, "embedded_mode")` (şifre kavramı yok)
//! - `get_app_paths` → yapıcıda verilen klasör yolları
//! - `shutdown` → session'ı durdurur
#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use librqbit::api::TorrentIdOrHash;
use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session, TorrentStatsState};
use manager_bridge::{BridgeError, ProgressEvent, TorrentBackend};
use manager_core::extract::ExtractHint;
use serde_json::{json, Value};
use tokio::sync::RwLock;

/// `rgsx+torrent://download?source=<url>&path=<p>&index=<i>` sarmalayıcısını çözer.
/// Döner: (gerçek torrent URL'i, seçim seçenekleri). Sarmalayıcı değilse URL değişmez,
/// varsayılan seçenekler döner. Çok dosyalı torrentlerde yalnız hedef dosya indirilir
/// (librqbit `only_files_regex` / `only_files`). Böylece "unsupported URL" hatası gider
/// ve Myrient/Redump gibi tek dosyalı torrentler tüm paketi indirmez.
fn resolve_rgsx_torrent(url: &str) -> (String, AddTorrentOptions) {
    if url.starts_with("rgsx+torrent:") {
        if let Some(q) = url.find('?') {
            let mut source = String::new();
            let mut path = String::new();
            let mut index: Option<usize> = None;
            for pair in url[q + 1..].split('&') {
                if let Some((k, v)) = pair.split_once('=') {
                    let val = percent_decode(v);
                    match k {
                        "source" => source = val,
                        "path" => path = val,
                        "index" => index = val.parse::<usize>().ok(),
                        _ => {}
                    }
                }
            }
            if !source.is_empty() {
                let mut opts = AddTorrentOptions::default();
                // Yeniden denemelerde/öturum kalıntısında ara dosya zaten var olabilir;
                // üzerine yazmaya izin ver (indirme yöneticisi için güvenli).
                opts.overwrite = true;
                if !path.is_empty() {
                    // Kök klasör adı farklarını yutması için yalnız dosya adıyla eşle.
                    let base = path.rsplit('/').next().unwrap_or(&path);
                    opts.only_files_regex = Some(format!(".*{}$", regex_escape(base)));
                } else if let Some(i) = index {
                    opts.only_files = Some(vec![i]);
                }
                return (source, opts);
            }
        }
    }
    (url.to_string(), AddTorrentOptions::default())
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_val(bytes[i + 1]);
            let lo = hex_val(bytes[i + 2]);
            if hi != 0xFF && lo != 0xFF {
                out.push((hi << 4) | lo);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_val(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 10,
        b'A'..=b'F' => c - b'A' + 10,
        _ => 0xFF,
    }
}

fn regex_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '|' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// lib.rs: manager-torrent — librqbit tabanlı embedded torrent engine.
pub struct LibrqbitEngine {
    /// Lazy spawn edilen librqbit session'ı (ensure_running'a kadar None).
    session: RwLock<Option<Arc<Session>>>,
    /// Torrent verilerinin yazılacağı kök klasör.
    output_folder: PathBuf,
    /// `get_app_paths` — indirme klasörü (tray "Downloads" için).
    downloads_folder: String,
    /// `get_app_paths` — log klasörü (tray "Logs" için).
    logs_folder: String,
    /// Aktif indirme handle'ları — task_id → handle (Gap-2 pause/resume için).
    active_handles: RwLock<HashMap<String, Arc<ManagedTorrent>>>,
}

impl std::fmt::Debug for LibrqbitEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibrqbitEngine")
            .field("output_folder", &self.output_folder)
            .finish_non_exhaustive()
    }
}

impl LibrqbitEngine {
    /// `downloads_folder` tray "Downloads" eylemi için; `logs_folder` tray "Logs" için.
    pub fn new(output_folder: PathBuf, downloads_folder: String, logs_folder: String) -> Self {
        Self {
            session: RwLock::new(None),
            output_folder,
            downloads_folder,
            logs_folder,
            active_handles: RwLock::new(HashMap::new()),
        }
    }

    /// Session zaten ayakta mı (ensure_running'in güçlü okuması).
    pub async fn is_running(&self) -> bool {
        self.session.read().await.is_some()
    }

    /// Lazy spawn: yoksa yeni librqbit `Session` oluşturur.
    /// Embedded kullanım: DHT + ileri bağlantılar çalışır, persistans/WebUI yok.
    pub async fn ensure_running(&self) -> Result<Arc<Session>, BridgeError> {
        if let Some(session) = self.session.read().await.as_ref() {
            return Ok(session.clone());
        }
        let session = Session::new(self.output_folder.clone())
            .await
            .map_err(|e| BridgeError::Rpc {
                code: -32000,
                message: format!("librqbit session kurulamadı: {e}"),
            })?;
        *self.session.write().await = Some(session.clone());
        Ok(session)
    }

    /// Bir `.torrent` dosyasını/magnet'ini session'a ekler (metadata+indirme başlatılır).
    /// `wait_until_completed` çağıran tarafın kontrolündedir.
    pub async fn add_torrent(
        &self,
        source: AddTorrent<'_>,
        opts: AddTorrentOptions,
    ) -> Result<Arc<ManagedTorrent>, BridgeError> {
        let session = self.ensure_running().await?;
        session
            .add_torrent(source, Some(opts))
            .await
            .map_err(|e| BridgeError::Rpc {
                code: -32000,
                message: format!("torrent eklenemedi: {e}"),
            })?
            .into_handle()
            .ok_or_else(|| BridgeError::Rpc {
                code: -32000,
                message: "torrent list-only modda eklendi, handle yok".to_string(),
            })
    }

    /// Üst seviye indirme akışı — Python `download_torrent_via_qbittorrent`'in
    /// librqbit karşılığı (bridge-qup tarafı: `source` magnet veya `.torrent
    /// URI'si; session'a ekler, tamamlanıncaya kadar bekler, indirilen dosyayı
    /// `output_folder` altında çözer ve `dest_path`'e hard-link/kopya yapar).
    ///
    /// Not: kapsamlı progres/seçim/seed takibi (`tag`, `temp_dir`,
    /// file-selection, `_seed_status_worker`) TASK-002f dışında kaldı — bu cep
    /// senkron "indir → çıkar" işlemini sunar. Canlı progress için
    /// `download_torrent_source_with_progress` kullanılır (TASK-002m).
    pub async fn download_torrent_source(
        &self,
        source: AddTorrent<'_>,
        dest_path: &std::path::Path,
        extract_hint: Option<ExtractHint>,
        opts: AddTorrentOptions,
    ) -> Result<std::path::PathBuf, BridgeError> {
        self.download_torrent_source_with_progress(source, dest_path, None, None, extract_hint, opts).await
    }

    /// `download_torrent_source`'un canlı progress yayınlayan hali (TASK-002m).
    /// `on_progress` varsa indirme sırasında `handle.stats()` döngüsünden
    /// `ProgressEvent` yayar; bitince `wait_until_completed` ile hash-check
    /// tamamlanır ve son bir `finished: true` olayı gönderilir.
    ///
    /// Gap-2: `task_id` verilirse handle `active_handles`'a kaydedilir (pause/resume
    /// için) ve indirme bitince/hatalanınca kaldırılır. Torrent `Paused` ise
    /// `paused: true` + speed 0 olayı yayar.
    pub async fn download_torrent_source_with_progress(
        &self,
        source: AddTorrent<'_>,
        dest_path: &std::path::Path,
        task_id: Option<String>,
        on_progress: Option<Arc<dyn Fn(ProgressEvent) + Send + Sync>>,
        extract_hint: Option<ExtractHint>,
        opts: AddTorrentOptions,
    ) -> Result<std::path::PathBuf, BridgeError> {
        // Çok dosyalı torrentlerde `handle.stats().total_bytes` TÜM torrent'tür (ör. 6TB);
        // `only_files`/`only_files_regex` ile yalnız seçili dosya indirilir, gerçek yazılacak
        // boyut çok daha küçüktür. Seçim aktifse alan kontrolünü atla (yalnız yazılabilirlik
        // kalır) — aksi halde seçili tek dosya için yanlış "disk alanı yetersiz" hatası verir.
        let selected = opts.only_files.is_some() || opts.only_files_regex.is_some();
        let handle = self.add_torrent(source, opts).await?;

        // Gap-5 (A+B): indirme başı disk alanı + yazılabilirlik ön-kontrolü.
        // Boyutu add_torrent sonrası biliyoruz (Python H8 parity). QueryFailed → atla.
        let dest_dir = dest_path.parent().unwrap_or(dest_path);
        let required = if selected { 0 } else { handle.stats().total_bytes };
        match manager_core::disk::precheck_destination(dest_dir, required) {
            Ok(()) => {}
            Err(manager_core::disk::DiskError::QueryFailed(_)) => {}
            Err(manager_core::disk::DiskError::PermissionDenied(m)) => {
                return Err(BridgeError::PermissionDenied(m))
            }
            Err(manager_core::disk::DiskError::InsufficientSpace { free, required }) => {
                return Err(BridgeError::DiskSpace(format!(
                    "gerekli {required} bayt, mevcut {free} bayt"
                )))
            }
        }

        if let Some(id) = &task_id {
            self.active_handles.write().await.insert(id.clone(), handle.clone());
        }
        let mut finished = false;
        let mut last_total = 0u64;
        let result = async {
            while !finished {
                // Gap-3: cancel edildiyse (handle `active_handles`'tan çıkarıldıysa)
                // döngüyü kır — `wait_until_completed`'e takılıp kalmayalım.
                if let Some(id) = &task_id {
                    if !self.active_handles.read().await.contains_key(id) {
                        return Err(BridgeError::Rpc {
                            code: -32000,
                            message: "indirme iptal edildi".to_string(),
                        });
                    }
                }
                let s = handle.stats();
                last_total = s.total_bytes;
                if let Some(cb) = &on_progress {
                    let paused = matches!(s.state, TorrentStatsState::Paused);
                    cb(ProgressEvent {
                        downloaded: s.progress_bytes,
                        total: s.total_bytes,
                        speed: if paused {
                            0.0
                        } else {
                            s.live.as_ref().map(|l| l.download_speed.mbps).unwrap_or(0.0)
                        },
                        finished: false,
                        paused,
                    });
                }
                finished = s.finished;
                if !finished {
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                }
            }
            handle
                .wait_until_completed()
                .await
                .map_err(|e| BridgeError::Rpc {
                    code: -32000,
                    message: format!("indirme tamamlanamadı: {e}"),
                })
        }
        .await;

        // Kayıttan çıkar (biten, iptal edilen veya hataya düşen indirme).
        if let Some(id) = &task_id {
            self.active_handles.write().await.remove(id);
        }
        result?;

        // İnen root'u çöz: bazı torrentlerin kök klasörü olabilir.
        let found = self.resolve_downloaded_file().await?;
        link_or_copy(&found, dest_path).map_err(|e| BridgeError::Rpc {
            code: -32000,
            message: format!("dosya sonlandırılamadı ({found:?} → {dest_path:?}): {e}"),
        })?;

        // GAP-6: indirme sonrası zorunlu arşiv açma (BIOS/PS3 redump /
        // is_zip_non_supported parity). `extract_hint` yoksa atlanır — API
        // katmanı platform bilgisini geçirir.
        if let Some(hint) = &extract_hint {
            run_post_download_extract(dest_path, dest_dir, hint).await?;
        }

        if let Some(cb) = &on_progress {
            cb(ProgressEvent {
                downloaded: last_total,
                total: last_total,
                speed: 0.0,
                finished: true,
                paused: false,
            });
        }
        tracing::info!(src = %found.display(), dst = %dest_path.display(), "torrent indirildi");
        Ok(found)
    }

    /// `pause_all` için gereken session + kayıtlı handle listesini toplar.
    async fn session_handles(
        &self,
    ) -> Result<(Option<Arc<Session>>, Vec<Arc<ManagedTorrent>>), BridgeError> {
        let handles: Vec<_> = self.active_handles.read().await.values().cloned().collect();
        if handles.is_empty() {
            // Aktif indirme yok — session spawn etmeye gerek yok (çevrimdışı birim
            // testlerinde DHT persistent kurulumu tetiklenmez).
            return Ok((None, handles));
        }
        let session = self.ensure_running().await?;
        Ok((Some(session), handles))
    }

    /// Tüm aktif indirmeleri duraklatır; duraklatılan sayıyı döner (Gap-2 `P1`).
    pub async fn pause_active(&self) -> Result<usize, BridgeError> {
        let (session, handles) = self.session_handles().await?;
        let Some(session) = session else { return Ok(0) };
        let mut n = 0usize;
        for h in &handles {
            if session.pause(h).await.is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// Duraklatılmış tüm indirmeleri sürdürür; sürdürülen sayıyı döner (Gap-2 `P2`).
    pub async fn resume_active(&self) -> Result<usize, BridgeError> {
        let (session, handles) = self.session_handles().await?;
        let Some(session) = session else { return Ok(0) };
        let mut n = 0usize;
        for h in &handles {
            if session.unpause(h).await.is_ok() {
                n += 1;
            }
        }
        Ok(n)
    }

    /// `task_id`'li tek indirmeyi duraklatır (Gap-2 `P0`).
    pub async fn pause_task(&self, task_id: &str) -> Result<bool, BridgeError> {
        let Some(h) = self.active_handles.read().await.get(task_id).cloned() else {
            return Ok(false);
        };
        let session = self.ensure_running().await?;
        session.pause(&h).await.map(|_| true).map_err(|e| BridgeError::Rpc {
            code: -32000,
            message: format!("pause başarısız ({task_id}): {e}"),
        })
    }

    /// `task_id`'li tek indirmeyi sürdürür (Gap-2).
    pub async fn resume_task(&self, task_id: &str) -> Result<bool, BridgeError> {
        let Some(h) = self.active_handles.read().await.get(task_id).cloned() else {
            return Ok(false);
        };
        let session = self.ensure_running().await?;
        session.unpause(&h).await.map(|_| true).map_err(|e| BridgeError::Rpc {
            code: -32000,
            message: format!("resume başarısız ({task_id}): {e}"),
        })
    }

    /// `task_id`'li indirme şu an duraklatılmış mı (Gap-2 `is_paused`).
    pub async fn is_task_paused(&self, task_id: &str) -> bool {
        self.active_handles
            .read()
            .await
            .get(task_id)
            .map(|h| matches!(h.stats().state, TorrentStatsState::Paused))
            .unwrap_or(false)
    }

    /// `task_id`'li tek indirmeyi iptal eder (Gap-3, Python `request_cancel` karşılığı).
    ///
    /// Handle'ı `active_handles`'tan çıkarıp `Session::delete(delete_files=true)`
    /// ile session'dan siler — librqbit `.rqbitpart`/kısmi dosyaları diskten de
    /// temizler. Progress loop'u bir sonraki turda map'ten çıktığını görüp iptal
    /// hatası döner. Dönen değer: task bulundu mu.
    pub async fn cancel_task(&self, task_id: &str) -> Result<bool, BridgeError> {
        let Some(handle) = self.active_handles.read().await.get(task_id).cloned() else {
            return Ok(false);
        };
        let session = self.ensure_running().await?;
        let id = TorrentIdOrHash::Id(handle.id());
        session.delete(id, true).await.map_err(|e| BridgeError::Rpc {
            code: -32000,
            message: format!("iptal başarısız ({task_id}): {e}"),
        })?;
        self.active_handles.write().await.remove(task_id);
        Ok(true)
    }

    /// Tüm aktif indirmeleri iptal eder; iptal edilen sayıyı döner (Gap-3
    /// `cancel_all_downloads` karşılığı).
    pub async fn cancel_all_tasks(&self) -> Result<usize, BridgeError> {
        let task_ids: Vec<String> = self.active_handles.read().await.keys().cloned().collect();
        let mut n = 0usize;
        for id in task_ids {
            if self.cancel_task(&id).await.unwrap_or(false) {
                n += 1;
            }
        }
        Ok(n)
    }

    /// `output_folder` içinde tamamlanmış torrent dosyasını bulur (Python
    /// `_resolve_downloaded_file` karşılığı). Torrent'teki ana içerik en büyük
    /// ve en anlamlı dosya olduğundan (`Sintel.mp4` vs `Sintel.de.srt` gibi)
    /// en büyük bağımsız içerik dosyası seçilir; `.rqbitpart` atlanır.
    pub async fn resolve_downloaded_file(&self) -> Result<std::path::PathBuf, BridgeError> {
        let root = self.output_folder.clone();
        let mut largest: Option<(u64, std::path::PathBuf)> = None;
        let mut stack = vec![root.clone()];
        while let Some(dir) = stack.pop() {
            let entries = std::fs::read_dir(&dir).map_err(|e| BridgeError::Rpc {
                code: -32000,
                message: format!("output klasörü okunamadı ({dir:?}): {e}"),
            })?;
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.is_file() {
                    let is_rqbit_part = p
                        .file_name()
                        .map(|n| n.to_string_lossy().ends_with(".rqbitpart"))
                        .unwrap_or(false);
                    if is_rqbit_part {
                        continue;
                    }
                    let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
                    if largest.as_ref().map(|(l, _)| len > *l).unwrap_or(true) {
                        largest = Some((len, p));
                    }
                }
            }
        }
        largest
            .map(|(_, p)| p)
            .ok_or_else(|| BridgeError::Rpc {
                code: -32000,
                message: "indirilen dosya bulunamadı".to_string(),
            })
    }
}

/// `src` dosyasına `dst`'ye hard-link dener; aynı dizin/yetki kısıtı olursa
/// `copy2`'e düşer (Python `os.link`/`shutil.copy2` karşılığı).
pub fn link_or_copy(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if dst.exists() {
        std::fs::remove_file(dst)?;
    }
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::hard_link(src, dst).or_else(|_| std::fs::copy(src, dst).map(|_| ()))
}

/// GAP-6 — indirme sonrası arşiv açma kararı + uygulaması (unit-test edilebilir).
///
/// `extract_hint` `None` ise veya `should_force_extract` false ise extract
/// atlanır (indirme başarılı sayılır). Zorunlu extract durumunda:
/// - Başarılı açma → bilgi logu.
/// - PS3 ISO decrypt / desteklenmeyen format (RAR) → indirme başarısız sayılmaz,
///   uyarı ile atlanır (kapsam dışı parity).
/// - Bozuk arşiv → `BridgeError::Extract` (BadZipFile parity, `FAILED_PERMANENT`).
pub(crate) async fn run_post_download_extract(
    dest_path: &std::path::Path,
    dest_dir: &std::path::Path,
    hint: &ExtractHint,
) -> Result<(), BridgeError> {
    let force = manager_core::extract::should_force_extract(
        hint.auto_extract,
        hint.is_zip_non_supported,
        &hint.platform_folder,
        &hint.platform,
    );
    if !force {
        return Ok(());
    }
    match manager_core::extract::extract_archive(dest_path, dest_dir) {
        Ok(outcome) => {
            tracing::info!(
                files = outcome.extracted_files,
                dst = %dest_path.display(),
                "GAP-6: arşiv otomatik açıldı"
            );
            Ok(())
        }
        // Bilinçli kapsam dışı → indirme başarısız sayılmaz, uyarı ile atlanır.
        Err(manager_core::extract::ExtractError::Ps3DecryptUnsupported) => {
            tracing::warn!(
                dst = %dest_path.display(),
                "GAP-6: PS3 ISO şifre çözme Rust'ta desteklenmiyor; extract atlandı"
            );
            Ok(())
        }
        Err(manager_core::extract::ExtractError::UnsupportedFormat { ext }) => {
            tracing::warn!(
                dst = %dest_path.display(),
                "GAP-6: desteklenmeyen arşiv formatı ({ext}); extract atlandı"
            );
            Ok(())
        }
        // Bozuk arşiv → gerçek hata (BadZipFile parity, FAILED_PERMANENT).
        Err(e) => Err(BridgeError::Extract(format!(
            "arşiv açılamadı ({}): {e}",
            dest_path.display()
        ))),
    }
}

#[async_trait::async_trait]
impl TorrentBackend for LibrqbitEngine {
    fn engine(&self) -> &'static str {
        "librqbit"
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        match method {
            "ping" => Ok(json!("pong")),
            "status" => Ok(json!({
                "state": if self.is_running().await { "RUNNING" } else { "STOPPED" },
                "available": true,
            })),
            "is_available" => Ok(json!(true)),
            "ensure_running" => {
                let _ = params.get("timeout");
                self.ensure_running().await.map(|_| json!(true))
            }
            "get_webui_url" => Ok(json!("")),
            "get_password_status" => Ok(json!({
                "available": true,
                "using_default": false,
                "secured": true,
                "mode": "embedded",
                "webui_url": "",
            })),
            "change_webui_password" => {
                let _ = params.get("password");
                Ok(json!([false, "embedded_mode"]))
            }
            "get_app_paths" => Ok(json!({
                "downloads_folder": self.downloads_folder,
                "logs_folder": self.logs_folder,
            })),
            "download_torrent" => {
                // JSON-RPC kontraktı (Python `_BRIDGE_METHODS` simetrisi) — trait
                // method'una aynı parametrelerle proxy eder.
                let source = params.get("source_url").and_then(Value::as_str).unwrap_or_default();
                let dest = params.get("dest_path").and_then(Value::as_str).unwrap_or_default();
                let hint = params
                    .get("extract_hint")
                    .and_then(Value::as_object)
                    .map(|o| ExtractHint {
                        auto_extract: o.get("auto_extract").and_then(Value::as_bool).unwrap_or(false),
                        is_zip_non_supported: o
                            .get("is_zip_non_supported")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                        platform_folder: o
                            .get("platform_folder")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                        platform: o
                            .get("platform")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    });
                match self.download_torrent(source, std::path::Path::new(dest), hint).await {
                    Ok(p) => Ok(json!(p.to_string_lossy().to_string())),
                    Err(e) => Err(e),
                }
            }
            "pause_all" => self.pause_all().await.map(|n| json!({ "paused": n })),
            "resume_all" => self.resume_all().await.map(|n| json!({ "resumed": n })),
            "pause" => {
                let id = params.get("task_id").and_then(Value::as_str).unwrap_or_default();
                self.pause_torrent(id).await.map(|_| json!(null))
            }
            "resume" => {
                let id = params.get("task_id").and_then(Value::as_str).unwrap_or_default();
                self.resume_torrent(id).await.map(|_| json!(null))
            }
            "is_paused" => {
                let id = params.get("task_id").and_then(Value::as_str).unwrap_or_default();
                self.is_paused(id).await.map(|b| json!(b))
            }
            "cancel" => {
                let id = params.get("task_id").and_then(Value::as_str).unwrap_or_default();
                self.cancel_task(id).await.map(|b| json!(b))
            }
            "cancel_all" => self.cancel_all_tasks().await.map(|n| json!({ "canceled": n })),
            "shutdown" => {
                if let Some(session) = self.session.write().await.take() {
                    session.stop().await;
                }
                Ok(json!(null))
            }
            other => Err(BridgeError::Rpc {
                code: -32601,
                message: format!("Method not found: {other}"),
            }),
        }
    }

    async fn shutdown(&self) {
        if let Some(session) = self.session.write().await.take() {
            session.stop().await;
        }
    }

    /// `download_torrent` → `source_url`'yi (magnet veya `.torrent` adresi)
    /// `AddTorrent::from_url` ile kurup senkron indirir, sonucu `dest_path`'e
    /// link/kopyalar (Python `download_torrent_via_qbittorrent` karşılığı).
    async fn download_torrent(
        &self,
        source_url: &str,
        dest_path: &std::path::Path,
        extract_hint: Option<ExtractHint>,
    ) -> Result<std::path::PathBuf, BridgeError> {
        let (real_url, opts) = resolve_rgsx_torrent(source_url);
        self.download_torrent_source(
            AddTorrent::from_url(real_url),
            dest_path,
            extract_hint,
            opts,
        )
        .await
    }

    /// TASK-002m: canlı progress akışı — `download_torrent_source_with_progress`
    /// üzerinden `handle.stats()` döngüsünden `ProgressEvent` yayar. Gap-2:
    /// `task_id` verilirse handle pause/resume kaydına alınır.
    async fn download_torrent_progress(
        &self,
        source_url: &str,
        dest_path: &std::path::Path,
        task_id: Option<String>,
        on_progress: Option<Arc<dyn Fn(ProgressEvent) + Send + Sync>>,
        extract_hint: Option<ExtractHint>,
    ) -> Result<std::path::PathBuf, BridgeError> {
        let (real_url, opts) = resolve_rgsx_torrent(source_url);
        self.download_torrent_source_with_progress(
            AddTorrent::from_url(real_url),
            dest_path,
            task_id,
            on_progress,
            extract_hint,
            opts,
        )
        .await
    }

    /// Gap-2 `P1`: tüm aktif indirmeleri duraklatır.
    async fn pause_all(&self) -> Result<usize, BridgeError> {
        self.pause_active().await
    }

    /// Gap-2 `P2`: duraklatılmış tüm indirmeleri sürdürür.
    async fn resume_all(&self) -> Result<usize, BridgeError> {
        self.resume_active().await
    }

    /// Gap-2 `P0`: tek indirmeyi duraklatır.
    async fn pause_torrent(&self, task_id: &str) -> Result<(), BridgeError> {
        let _ = self.pause_task(task_id).await?;
        Ok(())
    }

    /// Gap-2: tek indirmeyi sürdürür.
    async fn resume_torrent(&self, task_id: &str) -> Result<(), BridgeError> {
        let _ = self.resume_task(task_id).await?;
        Ok(())
    }

    /// Gap-2: `task_id`'li indirme duraklatılmış mı.
    async fn is_paused(&self, task_id: &str) -> Result<bool, BridgeError> {
        Ok(self.is_task_paused(task_id).await)
    }

    /// Gap-3: `task_id`'li indirmeyi iptal eder (kısmi/temp dosyaları da silinir).
    async fn cancel_torrent(&self, task_id: &str) -> Result<bool, BridgeError> {
        self.cancel_task(task_id).await
    }

    /// Gap-3: tüm aktif indirmeleri iptal eder; iptal edilen sayıyı döner.
    async fn cancel_all(&self) -> Result<usize, BridgeError> {
        self.cancel_all_tasks().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use manager_core::extract::ExtractHint;
    use std::io::Write;

    fn tmp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "rgsx_gap6_{}_{}_{}",
            label,
            std::process::id(),
            uuid_part()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn uuid_part() -> u64 {
        // Çakışmayı önlemek için basit bir NS sayaç (testler paralel koşabilir).
        use std::sync::atomic::{AtomicU64, Ordering};
        static C: AtomicU64 = AtomicU64::new(0);
        C.fetch_add(1, Ordering::Relaxed)
    }

    fn write_file(path: &std::path::Path, bytes: &[u8]) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    fn make_zip(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        use zip::write::SimpleFileOptions;
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, opts).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    /// `ExtractHint` gönderildiğinde (force=true) arşiv hedef dizine açılır.
    #[tokio::test]
    async fn gap6_hint_forces_archive_extraction_to_target() {
        let dir = tmp_dir("extract");
        let zip = dir.join("game.zip");
        make_zip(&zip, &[("roms/foo.bin", b"AAAA"), ("readme.txt", b"BBBB")]);
        let dest_dir = dir.join("out");
        let hint = ExtractHint {
            auto_extract: true,
            is_zip_non_supported: true,
            platform_folder: "snes".to_string(),
            platform: "Super Nintendo".to_string(),
        };
        assert!(manager_core::extract::should_force_extract(
            hint.auto_extract,
            hint.is_zip_non_supported,
            &hint.platform_folder,
            &hint.platform,
        ));

        let res = run_post_download_extract(&zip, &dest_dir, &hint).await;
        assert!(res.is_ok(), "force extract hata vermemeli: {res:?}");
        assert!(
            dest_dir.join("roms/foo.bin").is_file(),
            "arşiv hedef dizine açılmalı"
        );
        assert!(dest_dir.join("readme.txt").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `ExtractHint` yoksa / force=false ise extract atlanır (indirme başarılı).
    #[tokio::test]
    async fn gap6_no_hint_or_no_force_skips_extraction() {
        let dir = tmp_dir("skip");
        let zip = dir.join("game.zip");
        make_zip(&zip, &[("a.bin", b"X")]);
        let dest_dir = dir.join("out");

        // hint yok → atlanır
        let res = run_post_download_extract(&zip, &dest_dir, &ExtractHint::default()).await;
        assert!(res.is_ok());
        assert!(
            !dest_dir.join("a.bin").exists(),
            "force=false iken extract atlanmalı"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Desteklenmeyen format (RAR) → indirme başarısız sayılmaz, uyarı ile atlanır.
    #[tokio::test]
    async fn gap6_unsupported_format_does_not_fail_download() {
        let dir = tmp_dir("unsupported");
        let rar = dir.join("game.rar");
        write_file(&rar, b"rar-data");
        let dest_dir = dir.join("out");
        let hint = ExtractHint {
            auto_extract: true,
            is_zip_non_supported: true,
            platform_folder: "snes".to_string(),
            platform: "Super Nintendo".to_string(),
        };
        let res = run_post_download_extract(&rar, &dest_dir, &hint).await;
        assert!(res.is_ok(), "RAR desteklenmiyor ama indirme başarısız olmamalı");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Çıkarılamaz/kötü arşiv → `BridgeError::Extract` (FAILED_PERMANENT) döner.
    #[tokio::test]
    async fn gap6_corrupt_archive_returns_extract_error() {
        let dir = tmp_dir("corrupt");
        let zip = dir.join("bad.zip");
        write_file(&zip, b"this is not a zip");
        let dest_dir = dir.join("out");
        let hint = ExtractHint {
            auto_extract: true,
            is_zip_non_supported: true,
            platform_folder: "snes".to_string(),
            platform: "Super Nintendo".to_string(),
        };
        let res = run_post_download_extract(&zip, &dest_dir, &hint).await;
        assert!(
            matches!(res, Err(BridgeError::Extract(_))),
            "bozuk arşiv Extract hatası vermeli, geldi: {res:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}