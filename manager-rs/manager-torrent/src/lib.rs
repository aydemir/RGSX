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
use serde_json::{json, Value};
use tokio::sync::RwLock;

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
    ) -> Result<Arc<ManagedTorrent>, BridgeError> {
        let session = self.ensure_running().await?;
        session
            .add_torrent(source, Some(AddTorrentOptions::default()))
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
    ) -> Result<std::path::PathBuf, BridgeError> {
        self.download_torrent_source_with_progress(source, dest_path, None, None).await
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
    ) -> Result<std::path::PathBuf, BridgeError> {
        let handle = self.add_torrent(source).await?;

        // Gap-5 (A+B): indirme başı disk alanı + yazılabilirlik ön-kontrolü.
        // Boyutu add_torrent sonrası biliyoruz (Python H8 parity). QueryFailed → atla.
        let dest_dir = dest_path.parent().unwrap_or(dest_path);
        let required = handle.stats().total_bytes;
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
                match self.download_torrent(source, std::path::Path::new(dest)).await {
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
    ) -> Result<std::path::PathBuf, BridgeError> {
        self.download_torrent_source(AddTorrent::from_url(source_url.to_string()), dest_path)
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
    ) -> Result<std::path::PathBuf, BridgeError> {
        self.download_torrent_source_with_progress(
            AddTorrent::from_url(source_url.to_string()),
            dest_path,
            task_id,
            on_progress,
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