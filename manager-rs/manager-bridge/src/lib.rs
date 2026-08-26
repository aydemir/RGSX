//! manager-bridge: torrent engine sözleşmesi (`TorrentBackend`).
//!
//! TASK-013 (qBittorrent emekliliği): eski Python subprocess JSON-RPC istemcisi
//! (`Bridge`, `qbittorrent_backend.py --bridge`) söküldü. Crate artık yalnızca
//! engine-bağımsız sözleşmeyi taşır: librqbit engine'i (`manager-torrent`)
//! `TorrentBackend`'i uygular; manager-http `bridge_call` bu trait üzerinden
//! konuşur. Paylaşılan tipler: `BridgeError`, `ExtractHint` (manager-core),
//! `ProgressEvent`.
//!
//! Tarihsel not (arşiv): Python köprü protokolü `qbittorrent_backend.py::
//! _bridge_serve_loop`'ta yaşadı — satır-delimited JSON-RPC 2.0 (stdin/stdout),
//! id'siz `shutdown` bildirimi süreçyi bitirirdi. Geri dönüş için
//! `python-skeleton-final` tag'ine bakılmalı.

use std::sync::Arc;

use serde_json::{json, Value};

pub use manager_core::extract::ExtractHint;

/// İndirme ilerleme olayı — engine'den WebUI'ye canlı akış için.
///
/// `downloaded`/`total` bayt cinsinden; `speed` MiB/s; `finished` torrent'in
/// tamamlandığını (ve `download_torrent` sonlanmak üzere olduğunu) belirtir.
/// `paused` — Gap-2: torrent `TorrentStatsState::Paused` ise true (speed 0 raporlanır).
/// TASK-002m: librqbit engine'i `handle.stats()` döngüsünden bu olayı yayar.
#[derive(Debug, Clone, Copy)]
pub struct ProgressEvent {
    pub downloaded: u64,
    pub total: u64,
    pub speed: f64,
    pub finished: bool,
    pub paused: bool,
}

/// Köprü hatası.
#[derive(Debug)]
pub enum BridgeError {
    /// Süreç başlatılamadı (python/script bulunamadı).
    Spawn(String),
    /// stdin/stdout IO hatası.
    Io(std::io::Error),
    /// JSON-RPC hata yanıtı (Python tarafında exception).
    Rpc { code: i64, message: String },
    /// JSON parse / protokol ihlali.
    Protocol(String),
    /// Yanıt zaman aşımı (child cevap vermedi).
    Timeout(String),
    /// Yetersiz disk alanı (indirme öncesi ön-kontrol).
    DiskSpace(String),
    /// Hedef dizine yazma izni yok (indirme öncesi ön-kontrol).
    PermissionDenied(String),
    /// GAP-6 — indirme sonrası arşiv açma hatası (bozuk arşiv → FAILED_PERMANENT).
    Extract(String),
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Spawn(m) => write!(f, "bridge spawn: {m}"),
            BridgeError::Io(e) => write!(f, "bridge io: {e}"),
            BridgeError::Rpc { code, message } => write!(f, "bridge rpc error {code}: {message}"),
            BridgeError::Protocol(m) => write!(f, "bridge protocol: {m}"),
            BridgeError::Timeout(m) => write!(f, "bridge timeout: {m}"),
            BridgeError::DiskSpace(m) => write!(f, "bridge disk space: {m}"),
            BridgeError::PermissionDenied(m) => write!(f, "bridge permission denied: {m}"),
            BridgeError::Extract(m) => write!(f, "bridge extract: {m}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<std::io::Error> for BridgeError {
    fn from(e: std::io::Error) -> Self {
        BridgeError::Io(e)
    }
}

/// JSON-RPC köprü sözleşmesi — engine-bağımsız arayüz.
///
/// Faz 10b'den beri manager-http Python'a sabit bağlı değildir; `TorrentBackend`
///'i uygulayan her engine (günümüzde librqbit) aynı sözleşmeyi bağlar.
///
/// TASK-013: qBittorrent-kavramlı default metodlar (is_available/ensure_running/
/// get_webui_url/get_password_status/change_webui_password/
/// regenerate_qbittorrent_password/ping/status) söküldü — tek tüketicileri
/// emekli olan `/api/qbittorrent/*` uçlarıydı.
#[async_trait::async_trait]
pub trait TorrentBackend: Send + Sync + std::fmt::Debug {
    /// Motorun adı (`librqbit`) — log/health için.
    fn engine(&self) -> &'static str;

    /// JSON-RPC metod çağrısı — Python bridge ile aynı isim uzayı.
    async fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError>;

    /// Kapanış bildirimi (best effort).
    async fn shutdown(&self);

    /// Tüm aktif indirmeleri duraklatır (Gap-2, `P1` karşılığı).
    ///
    /// Default: `pause_all` JSON-RPC'sine proxy eder. librqbit
    /// engine gerçek implementasyonla override eder. Dönen değer duraklatılan sayıdır.
    async fn pause_all(&self) -> Result<usize, BridgeError> {
        let v = self.call("pause_all", json!({})).await?;
        Ok(v.get("paused").and_then(Value::as_u64).unwrap_or(0) as usize)
    }

    /// Duraklatılmış tüm indirmeleri sürdürür (Gap-2, `P2` karşılığı).
    ///
    /// Default: `resume_all` JSON-RPC'sine proxy eder. Dönen değer sürdürülen sayıdır.
    async fn resume_all(&self) -> Result<usize, BridgeError> {
        let v = self.call("resume_all", json!({})).await?;
        Ok(v.get("resumed").and_then(Value::as_u64).unwrap_or(0) as usize)
    }

    /// Tek bir indirmeyi duraklatır (Gap-2, `P0` — Python `toggle_pause_download`).
    ///
    /// Default: `pause` JSON-RPC'sine proxy eder. librqbit engine override eder.
    async fn pause_torrent(&self, task_id: &str) -> Result<(), BridgeError> {
        let _ = self.call("pause", json!({ "task_id": task_id })).await?;
        Ok(())
    }

    /// Duraklatılmış tek bir indirmeyi sürdürür (Gap-2).
    ///
    /// Default: `resume` JSON-RPC'sine proxy eder. librqbit engine override eder.
    async fn resume_torrent(&self, task_id: &str) -> Result<(), BridgeError> {
        let _ = self.call("resume", json!({ "task_id": task_id })).await?;
        Ok(())
    }

    /// `task_id`'li indirme şu an duraklatılmış mı (Gap-2 `is_paused`).
    ///
    /// Default: `is_paused` JSON-RPC'sine proxy eder; sonuç yoksa false.
    async fn is_paused(&self, task_id: &str) -> Result<bool, BridgeError> {
        let v = self
            .call("is_paused", json!({ "task_id": task_id }))
            .await?;
        Ok(v.as_bool().unwrap_or(false))
    }

    /// Tek bir indirmeyi iptal eder; kısmi/`.rqbitpart`/temp dosyalarını da siler
    /// (Gap-3, `P3..P6` karşılığı).
    ///
    /// Default: `cancel` JSON-RPC'sine proxy eder. librqbit engine override edip
    /// `Session::delete(id, delete_files=true)` üzerinden gerçek temizlik yapar.
    /// Dönen değer: iptal edilen task bulundu mu.
    async fn cancel_torrent(&self, task_id: &str) -> Result<bool, BridgeError> {
        let v = self.call("cancel", json!({ "task_id": task_id })).await?;
        Ok(v.as_bool().unwrap_or(false))
    }

    /// Tüm aktif indirmeleri iptal eder + kısmi/temp dosyalarını temizler
    /// (Gap-3, `cancel_all_downloads` karşılığı).
    ///
    /// Default: `cancel_all` JSON-RPC'sine proxy eder. Dönen değer iptal edilen
    /// indirme sayısıdır.
    async fn cancel_all(&self) -> Result<usize, BridgeError> {
        let v = self.call("cancel_all", json!({})).await?;
        Ok(v.get("canceled").and_then(Value::as_u64).unwrap_or(0) as usize)
    }

    /// `get_app_paths` → tray menüsü için indirme/log klasör yolları.
    async fn get_app_paths(&self) -> Result<(String, String), BridgeError> {
        let v = self.call("get_app_paths", json!({})).await?;
        let downloads = v
            .get("downloads_folder")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let logs = v
            .get("logs_folder")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Ok((downloads, logs))
    }

    /// `download_torrent` → `source_url` (magnet veya `.torrent` adresi) indirilir,
    /// sonuç `dest_path`'e hard-link/kopya ile sonlandırılır. Dönen yol indirilen
    /// kaynak dosyadır (engine içinde çözülen).
    async fn download_torrent(
        &self,
        source_url: &str,
        dest_path: &std::path::Path,
        extract_hint: Option<ExtractHint>,
    ) -> Result<std::path::PathBuf, BridgeError> {
        let hint = extract_hint
            .map(|h| {
                json!({
                    "auto_extract": h.auto_extract,
                    "is_zip_non_supported": h.is_zip_non_supported,
                    "platform_folder": h.platform_folder,
                    "platform": h.platform,
                })
            })
            .unwrap_or(Value::Null);
        let v = self
            .call(
                "download_torrent",
                json!({
                    "source_url": source_url,
                    "dest_path": dest_path.to_string_lossy().to_string(),
                    "extract_hint": hint,
                }),
            )
            .await?;
        let path = v.as_str().ok_or_else(|| BridgeError::Rpc {
            code: -32601,
            message: "download_torrent sonucu string yol değil".to_string(),
        })?;
        Ok(std::path::PathBuf::from(path))
    }

    /// `download_torrent` ile aynı akış, ama indirme **sırasında** `on_progress`
    /// callback'ine canlı ilerleme olayları yayar (varsa). WebUI progress bar'ını
    /// canlı beslemek için kullanılır.
    ///
    /// Default: `on_progress`'u yok sayar ve sıradan `download_torrent`'e düşer.
    /// librqbit engine override edip `handle.stats()` döngüsünden olay yayar.
    /// Gap-2: `task_id` verilirse engine pause/resume için handle'ı kaydeder.
    async fn download_torrent_progress(
        &self,
        source_url: &str,
        dest_path: &std::path::Path,
        _task_id: Option<String>,
        on_progress: Option<Arc<dyn Fn(ProgressEvent) + Send + Sync>>,
        extract_hint: Option<ExtractHint>,
    ) -> Result<std::path::PathBuf, BridgeError> {
        let _ = on_progress;
        self.download_torrent(source_url, dest_path, extract_hint)
            .await
    }
}
