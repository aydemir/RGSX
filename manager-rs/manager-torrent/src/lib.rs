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

use std::path::PathBuf;
use std::sync::Arc;

use librqbit::{AddTorrent, AddTorrentOptions, ManagedTorrent, Session};
use manager_bridge::TorrentBackend;
use manager_bridge::BridgeError;
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
    /// librqbit karşılığı (bridge-qup tarafı: `source` magnet veya `.torrent`
    /// URL'si; session'a ekler, tamamlanıncaya kadar bekler, indirilen dosyayı
    /// `output_folder` altında çözer ve `dest_path`'e hard-link/kopya yapar).
    ///
    /// Not: kapsamlı progres/seçim/seed takibi (`tag`, `temp_dir`,
    /// file-selection, `_seed_status_worker`) TASK-002f dışında kaldı — bu cep
    /// senkron "indir → çıkar" işlemini sunar.
    pub async fn download_torrent(
        &self,
        source: AddTorrent<'_>,
        dest_path: &std::path::Path,
    ) -> Result<std::path::PathBuf, BridgeError> {
        let handle = self.add_torrent(source).await?;
        handle
            .wait_until_completed()
            .await
            .map_err(|e| BridgeError::Rpc {
                code: -32000,
                message: format!("indirme tamamlanamadı: {e}"),
            })?;

        // İnen root'u çöz: bazı torrent'lerin kök klasörü olabilir.
        let found = self.resolve_downloaded_file().await?;
        link_or_copy(&found, dest_path).map_err(|e| BridgeError::Rpc {
            code: -32000,
            message: format!("dosya sonlandırılamadı ({found:?} → {dest_path:?}): {e}"),
        })?;
        tracing::info!(src = %found.display(), dst = %dest_path.display(), "torrent indirildi");
        Ok(found)
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
}