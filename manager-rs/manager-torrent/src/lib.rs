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