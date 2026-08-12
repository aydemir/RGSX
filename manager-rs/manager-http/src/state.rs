//! Paylaşılan HTTP durumu: `AppState` + `StateData`.
//!
//! TASK-002b — Python `config` modülünün web yüzeyindeki karşılığı
//! (`download_queue`, `download_progress`, `history`, `downloaded_games`).
//! placeholder: gerçek persist/worker bağlantısı TASK-002c.

use std::sync::{Arc, RwLock};
use tokio::sync::broadcast::Sender;

use manager_bridge::TorrentBackend;
use manager_core::state::ManagerState;

use crate::sse;
use serde_json::json;

pub const SNAPSHOT_KEYS: [&str; 5] = ["history", "queue", "active", "progress", "downloaded"];

/// İş mantığı durumu (config eşleniği). `RwLock` + senkron erişim — axum
/// handler'ları kısa tutulduğu için `tokio::sync`'e gerek yok.
#[derive(Debug)]
pub struct StateData {
    /// `config.history` — indirme geçmişi.
    pub history: Vec<serde_json::Value>,
    /// `config.download_queue` — kuyruktaki görevler.
    pub queue: Vec<serde_json::Value>,
    /// `config.download_progress` — ilerleme haritası (url -> yüzde/bytes).
    pub progress: serde_json::Value,
    /// `config.downloaded_games` — tamamlanan oyunlar.
    pub downloaded: serde_json::Value,
    /// `config.download_active` — aktif indirme bayrağı.
    pub active: bool,
    /// Manager durumu (watchdog state machine; varsayılan INIT).
    pub manager_state: ManagerState,
    /// Process id — `/api/health` `pid` alanı.
    pub pid: u32,
    /// `/api/settings` dönen app ayarları (placeholder).
    pub settings: serde_json::Value,
    /// `/api/system_info` — cihaz bilgileri (placeholder).
    pub system_info: serde_json::Value,
}

impl StateData {
    /// Boş state:`/api/health` için `pid > 0` garanti eder (contract testi).
    pub fn empty() -> Self {
        Self {
            history: vec![],
            queue: vec![],
            progress: serde_json::json!({}),
            downloaded: serde_json::json!({}),
            active: false,
            manager_state: ManagerState::Init,
            pid: std::process::id(),
            settings: serde_json::json!({}),
            system_info: serde_json::json!({}),
        }
    }

    /// `config.download_queue` uzunluğu.
    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }

    /// `/api/browse-directories` — verilen kökün alt dizinleri (placeholder: boş).
    /// Python `handlers_ui.py` gerçek tarama yapar; bu slice sadece şablon.
    pub fn browse(&self, path: &str) -> (String, Vec<serde_json::Value>) {
        let dirs: Vec<serde_json::Value> = std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| {
                        json!({
                            "name": e.file_name().to_string_lossy().to_string(),
                            "path": e.path().to_string_lossy().to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        (path.to_string(), dirs)
    }
}

/// Axum handler'lara `State` extractor ile verilen paylaşılan durum.
#[derive(Clone)]
pub struct AppState {
    pub data: Arc<RwLock<StateData>>,
    pub events: Sender<String>,
    /// Torrent backend (Python bridge subprocess veya in-process librqbit engine).
    /// manager-bin kurar; handler'lar buradan `call` yapar.
    pub bridge: Option<Arc<dyn TorrentBackend>>,
    /// WebUI statik dosyalarının kökü (`.../static/`). None ise statik servis kapalı.
    pub static_root: Option<std::path::PathBuf>,
}

impl AppState {
    /// Boş state + yeni SSE kanalı.
    pub fn empty() -> Self {
        Self {
            data: Arc::new(RwLock::new(StateData::empty())),
            events: sse::channel(),
            bridge: None,
            static_root: None,
        }
    }

    /// Kanalı paylaşırken (test) verilen sender ile kurar.
    pub fn with_data(data: StateData, events: Sender<String>) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
            events,
            bridge: None,
            static_root: None,
        }
    }

    /// bridge yoksa sahte `BridgeError::Spawn` döndürür (handler'lar placeholder
    /// davranışına düşer). Varsa `call`'ı proxy eder.
    pub async fn bridge_call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value, manager_bridge::BridgeError> {
        match &self.bridge {
            Some(b) => b.call(method, params).await,
            None => Err(manager_bridge::BridgeError::Spawn(
                "bridge başlatılmadı".to_string(),
            )),
        }
    }

    /// Okuma kilidi (poison'u unwrap — placeholder; Python config benzeri global).
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, StateData> {
        self.data.read().unwrap()
    }

    /// Yazma kilidi (poison'u unwrap).
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, StateData> {
        self.data.write().unwrap()
    }
}