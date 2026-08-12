//! manager-bridge: Python subprocess stdio JSON-RPC köprüsü.
//!
//! TASK-002c — `qbittorrent_backend.py --bridge`'i subprocess olarak başlatır ve
//! satır-delimited JSON-RPC 2.0 ile konuşur (stdin/stdout). Python downloader
//! mantığı yerinde kalır; Rust tarafı onu çağırır.
//!
//! Protokol (Python tarafı `qbittorrent_backend.py::_bridge_serve_loop`):
//! - Her satır tek JSON-RPC 2.0 mesajı.
//! - Yanıt satırı `{"jsonrpc":"2.0","id":<id>,"result":...}` veya error.
//! - `shutdown` bildirimi (id'siz) süreci bitirir.
//!
//! Örnek:
//! ```no_run
//! # use manager_bridge::{Bridge, BridgeConfig};
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let bridge = Bridge::spawn(BridgeConfig::default())?;
//! let _pong = bridge.ping().await?;
//! let _url = bridge.get_webui_url().await?;
//! bridge.shutdown().await;
//! Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::{oneshot, Mutex as AsyncMutex};

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
}

impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BridgeError::Spawn(m) => write!(f, "bridge spawn: {m}"),
            BridgeError::Io(e) => write!(f, "bridge io: {e}"),
            BridgeError::Rpc { code, message } => write!(f, "bridge rpc error {code}: {message}"),
            BridgeError::Protocol(m) => write!(f, "bridge protocol: {m}"),
            BridgeError::Timeout(m) => write!(f, "bridge timeout: {m}"),
        }
    }
}

impl std::error::Error for BridgeError {}

impl From<std::io::Error> for BridgeError {
    fn from(e: std::io::Error) -> Self {
        BridgeError::Io(e)
    }
}

/// Köprü sürecinin yapılandırması.
#[derive(Debug, Clone)]
pub struct BridgeConfig {
    /// Python yorumlayıcı yolu (`python`/`pythonw`).
    pub python: String,
    /// Köprü scriptinin mutlak yolu (qbittorrent_backend.py).
    pub script: String,
    /// İstek zaman aşımı (saniye).
    pub timeout_secs: u64,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            python: "python".to_string(),
            script: String::new(),
            timeout_secs: 30,
        }
    }
}

/// Bekleyen istek kaydı: id -> yanıt kanalı.
type Pending = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, BridgeError>>>>>;

/// Python bridge süreci üzerinde JSON-RPC istemcisi.
#[derive(Debug)]
pub struct Bridge {
    child: Child,
    stdin: Option<AsyncMutex<ChildStdin>>,
    pending: Pending,
    next_id: AtomicU64,
    config: BridgeConfig,
}

impl Bridge {
    /// `python <script> --bridge` sürecini başlatır ve stdout reader task'ini kurar.
    ///
    /// `RGSX_HEADLESS=1` set edilir: `config.py` import'u pygame banner/print'lerini
    /// susturur (JSON satırları kirletilmez).
    pub fn spawn(config: BridgeConfig) -> Result<Self, BridgeError> {
        if config.script.is_empty() {
            return Err(BridgeError::Spawn("script yolu boş".to_string()));
        }
        let mut child = Command::new(&config.python)
            .arg(&config.script)
            .arg("--bridge")
            .env("RGSX_HEADLESS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| BridgeError::Spawn(e.to_string()))?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take().ok_or_else(|| {
            BridgeError::Spawn("child stdout alınamadı".to_string())
        })?;
        // stderr'i okumayan tüketici — asılmaması için boşalttır (drain).
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(drain_stderr(stderr));
        }

        let pending: Pending = Arc::new(Mutex::new(HashMap::new()));
        let reader_pending = Arc::clone(&pending);
        tokio::spawn(read_loop(stdout, reader_pending));

        Ok(Self {
            child,
            stdin: stdin.map(AsyncMutex::new),
            pending,
            next_id: AtomicU64::new(1),
            config,
        })
    }

    /// Köprü script yolunu / python'ı dışarıdan alır (test, manager-bin).
    pub fn config(&self) -> &BridgeConfig {
        &self.config
    }

    /// JSON-RPC çağrısı: `method` + `params` → `result`.
    pub async fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().unwrap();
            pending.insert(id, tx);
        }

        let payload = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        self.write_line(&payload.to_string()).await?;

        let timeout = tokio::time::Duration::from_secs(self.config.timeout_secs);
        let result = tokio::time::timeout(timeout, rx)
            .await
            .map_err(|_| BridgeError::Timeout(format!("{method} ({id}) yanıt vermedi")))?
            .map_err(|_| BridgeError::Protocol("yanıt kanalı kapatıldı".to_string()))?;

        self.pending.lock().unwrap().remove(&id);
        result
    }

    async fn write_line(&self, line: &str) -> Result<(), BridgeError> {
        let stdin = self.stdin.as_ref().ok_or_else(|| {
            BridgeError::Io(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "stdin kapalı"))
        })?;
        let mut guard = stdin.lock().await;
        let mut buf = line.as_bytes().to_vec();
        buf.push(b'\n');
        guard.write_all(&buf).await?;
        guard.flush().await?;
        Ok(())
    }

    // -- Typed metodlar (Python public API karşılığı) ----------------------

    /// `ping` → `"pong"`.
    pub async fn ping(&self) -> Result<String, BridgeError> {
        let v = self.call("ping", json!({})).await?;
        Ok(v.as_str().unwrap_or_default().to_string())
    }

    /// `status` → `{state, available}`.
    pub async fn status(&self) -> Result<BridgeStatus, BridgeError> {
        let v = self.call("status", json!({})).await?;
        Ok(BridgeStatus {
            state: v.get("state").and_then(Value::as_str).unwrap_or_default().to_string(),
            available: v.get("available").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// `is_available` → fallback qBittorrent kullanılabilir mi.
    pub async fn is_available(&self) -> Result<bool, BridgeError> {
        Ok(self.call("is_available", json!({})).await?.as_bool().unwrap_or(false))
    }

    /// `ensure_running` → qBittorrent başlatıldı.
    pub async fn ensure_running(&self, timeout_secs: f64) -> Result<bool, BridgeError> {
        Ok(self
            .call("ensure_running", json!({ "timeout": timeout_secs }))
            .await?
            .as_bool()
            .unwrap_or(false))
    }

    /// `get_webui_url` → WebUI adresi.
    pub async fn get_webui_url(&self) -> Result<String, BridgeError> {
        Ok(self.call("get_webui_url", json!({})).await?.as_str().unwrap_or_default().to_string())
    }

    /// `get_password_status` → şifre durumu dict'i.
    pub async fn get_password_status(&self) -> Result<Value, BridgeError> {
        self.call("get_password_status", json!({})).await
    }

    /// `change_webui_password` → `(ok, message)`.
    pub async fn change_webui_password(&self, password: &str) -> Result<(bool, String), BridgeError> {
        let v = self.call("change_webui_password", json!({ "password": password })).await?;
        let arr = v.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let ok = arr.first().and_then(Value::as_bool).unwrap_or(false);
        let msg = arr.get(1).and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((ok, msg))
    }

    /// `regenerate_qbittorrent_password` → `(ok, password)` (yeni rastgele şifre).
    pub async fn regenerate_qbittorrent_password(&self) -> Result<(bool, String), BridgeError> {
        let v = self.call("regenerate_qbittorrent_password", json!({})).await?;
        let arr = v.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let ok = arr.first().and_then(Value::as_bool).unwrap_or(false);
        let pw = arr.get(1).and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((ok, pw))
    }

    /// `get_app_paths` → tray menüsü için indirme/log klasör yolları.
    pub async fn get_app_paths(&self) -> Result<(String, String), BridgeError> {
        let v = self.call("get_app_paths", json!({})).await?;
        let downloads = v.get("downloads_folder").and_then(Value::as_str).unwrap_or_default().to_string();
        let logs = v.get("logs_folder").and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((downloads, logs))
    }

    /// `shutdown` bildirimi — süreci kapatır (id'siz; yanıt yok).
    pub async fn shutdown(&self) {
        let payload = json!({ "jsonrpc": "2.0", "method": "shutdown" });
        let _ = self.write_line(&payload.to_string()).await;
    }

    /// Süreci bekle (kendiliğinden çıkış) — örn. shutdown sonrası.
    pub async fn wait(mut self) -> Option<std::process::ExitStatus> {
        self.child.wait().await.ok()
    }
}

/// `status` metodunun yapılandırılmış sonucu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BridgeStatus {
    pub state: String,
    pub available: bool,
}

/// JSON-RPC köprü sözleşmesi — hem Python subprocess (`Bridge`) hem in-process
/// engine'ler (librqbit) aynı metod adlarını ve yanıt şekillerini sunar.
///
/// Faz 10b: manager-http, Python `Bridge`'e sabit bağlı değildir; `TorrentBackend`
/// import eden her engine (ör. librqbit) aynı sözleşmeyi bağlar.
#[async_trait::async_trait]
pub trait TorrentBackend: Send + Sync + std::fmt::Debug {
    /// Motorun adı (`python`/`librqbit`) — log/health için.
    fn engine(&self) -> &'static str;

    /// JSON-RPC metod çağrısı — Python bridge ile aynı isim uzayı.
    async fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError>;

    /// Kapanış bildirimi (best effort).
    async fn shutdown(&self);

    /// `ping` → `"pong"`.
    async fn ping(&self) -> Result<String, BridgeError> {
        let v = self.call("ping", json!({})).await?;
        Ok(v.as_str().unwrap_or_default().to_string())
    }

    /// `status` → `{state, available}`.
    async fn status(&self) -> Result<BridgeStatus, BridgeError> {
        let v = self.call("status", json!({})).await?;
        Ok(BridgeStatus {
            state: v.get("state").and_then(Value::as_str).unwrap_or_default().to_string(),
            available: v.get("available").and_then(Value::as_bool).unwrap_or(false),
        })
    }

    /// `is_available` → fallback qBittorrent kullanılabilir mi.
    async fn is_available(&self) -> Result<bool, BridgeError> {
        Ok(self.call("is_available", json!({})).await?.as_bool().unwrap_or(false))
    }

    /// `ensure_running` → torrent engine başlatıldı.
    async fn ensure_running(&self, timeout_secs: f64) -> Result<bool, BridgeError> {
        Ok(self
            .call("ensure_running", json!({ "timeout": timeout_secs }))
            .await?
            .as_bool()
            .unwrap_or(false))
    }

    /// `get_webui_url` → WebUI adresi (librqbit'te boş/kendi adresi).
    async fn get_webui_url(&self) -> Result<String, BridgeError> {
        Ok(self.call("get_webui_url", json!({})).await?.as_str().unwrap_or_default().to_string())
    }

    /// `get_password_status` → şifre durumu dict'i.
    async fn get_password_status(&self) -> Result<Value, BridgeError> {
        self.call("get_password_status", json!({})).await
    }

    /// `change_webui_password` → `(ok, message)`.
    async fn change_webui_password(&self, password: &str) -> Result<(bool, String), BridgeError> {
        let v = self.call("change_webui_password", json!({ "password": password })).await?;
        let arr = v.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let ok = arr.first().and_then(Value::as_bool).unwrap_or(false);
        let msg = arr.get(1).and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((ok, msg))
    }

    /// `regenerate_qbittorrent_password` → `(ok, password)` (yeni rastgele şifre).
    async fn regenerate_qbittorrent_password(&self) -> Result<(bool, String), BridgeError> {
        let v = self.call("regenerate_qbittorrent_password", json!({})).await?;
        let arr = v.as_array().map(|a| a.as_slice()).unwrap_or(&[]);
        let ok = arr.first().and_then(Value::as_bool).unwrap_or(false);
        let pw = arr.get(1).and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((ok, pw))
    }

    /// `get_app_paths` → tray menüsü için indirme/log klasör yolları.
    async fn get_app_paths(&self) -> Result<(String, String), BridgeError> {
        let v = self.call("get_app_paths", json!({})).await?;
        let downloads = v.get("downloads_folder").and_then(Value::as_str).unwrap_or_default().to_string();
        let logs = v.get("logs_folder").and_then(Value::as_str).unwrap_or_default().to_string();
        Ok((downloads, logs))
    }

    /// `download_torrent` → `source_url` (magnet veya `.torrent` adresi) indirilir,
    /// sonuç `dest_path`'e hard-link/kopya ile sonlandırılır. Dönen yol indirilen
    /// kaynak dosyadır (engine içinde çözülen).
    ///
    /// Default: `call("download_torrent", {source_url, dest_path})` JSON-RPC'sine
    /// proxy eder — Python bridge'de aynı isimli `_BRIDGE_METHODS`'a karşılık gelir;
    /// librqbit engine yerel implementasyonla override eder.
    async fn download_torrent(
        &self,
        source_url: &str,
        dest_path: &std::path::Path,
    ) -> Result<std::path::PathBuf, BridgeError> {
        let v = self
            .call(
                "download_torrent",
                json!({
                    "source_url": source_url,
                    "dest_path": dest_path.to_string_lossy().to_string(),
                }),
            )
            .await?;
        let path = v.as_str().ok_or_else(|| BridgeError::Rpc {
            code: -32601,
            message: "download_torrent sonucu string yol değil".to_string(),
        })?;
        Ok(std::path::PathBuf::from(path))
    }
}

/// `Bridge`'i `TorrentBackend` sözleşmesine bağlar (Python subprocess motoru).
///
/// Mevcut davranış birebir korunur — yalnızca trait arayüzü üzerinden genelleşir.
#[async_trait::async_trait]
impl TorrentBackend for Bridge {
    fn engine(&self) -> &'static str {
        "python"
    }

    async fn call(&self, method: &str, params: Value) -> Result<Value, BridgeError> {
        Bridge::call(self, method, params).await
    }

    async fn shutdown(&self) {
        Bridge::shutdown(self).await;
    }
}

async fn drain_stderr(stderr: tokio::process::ChildStderr) {
    use tokio::io::AsyncReadExt;
    let mut reader = BufReader::new(stderr);
    let mut buf = [0u8; 1024];
    while reader.read(&mut buf).await.map(|n| n > 0).unwrap_or(false) {
        // stderr'i boşalt (reader yoksa child yazınca bloke olur).
    }
}

async fn read_loop(stdout: ChildStdout, pending: Pending) {
    let mut lines = BufReader::new(stdout).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let value: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                // JSON olmayan satır (örn. import print'leri) sessizce atlanır.
                eprintln!("manager-bridge: JSON olmayan satır atlandı: {e}");
                continue;
            }
        };
        let Some(id) = value.get("id").and_then(Value::as_u64) else {
            continue; // notification / parse error yanıtı — bekleyen yok
        };
        let result = if let Some(err) = value.get("error") {
            Err(BridgeError::Rpc {
                code: err.get("code").and_then(Value::as_i64).unwrap_or(-32000),
                message: err.get("message").and_then(Value::as_str).unwrap_or_default().to_string(),
            })
        } else {
            Ok(value.get("result").cloned().unwrap_or(Value::Null))
        };
        let tx = pending.lock().unwrap().remove(&id);
        if let Some(tx) = tx {
            let _ = tx.send(result);
        }
    }
    // stdout bitti — bekleyen istekleri kapat.
    let drained: Vec<_> = pending.lock().unwrap().drain().collect();
    for (_, tx) in drained {
        let _ = tx.send(Err(BridgeError::Protocol(
            "bridge stdout kapandı (süreç çıktı)".to_string(),
        )));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> BridgeConfig {
        BridgeConfig {
            python: "python".to_string(),
            script: "tests/echo_bridge.py".to_string(),
            timeout_secs: 15,
        }
    }

    #[tokio::test]
    async fn spawn_requires_script() {
        assert!(Bridge::spawn(BridgeConfig::default()).is_err());
    }

    #[tokio::test]
    async fn ping_echo_roundtrip() {
        let bridge = Bridge::spawn(cfg()).unwrap();
        assert_eq!(bridge.ping().await.unwrap(), "pong");
    }

    #[tokio::test]
    async fn status_and_typed_methods() {
        let bridge = Bridge::spawn(cfg()).unwrap();
        let st = bridge.status().await.unwrap();
        assert_eq!(st.state, "STOPPED");
        assert!(st.available);

        assert_eq!(bridge.get_webui_url().await.unwrap(), "http://localhost:18572/");
        let pw = bridge.get_password_status().await.unwrap();
        assert!(pw.is_object());

        let (ok, msg) = bridge.change_webui_password("x").await.unwrap();
        assert!(!ok);
        assert_eq!(msg, "password_too_short");
    }

    #[tokio::test]
    async fn unknown_method_returns_rpc_error() {
        let bridge = Bridge::spawn(cfg()).unwrap();
        let err = bridge.call("nope", json!({})).await.unwrap_err();
        match err {
            BridgeError::Rpc { code, .. } => assert_eq!(code, -32601),
            other => panic!("beklenen Rpc hatası, geldi: {other}"),
        }
    }

    #[tokio::test]
    async fn shutdown_notification_ends_process() {
        let bridge = Bridge::spawn(cfg()).unwrap();
        bridge.shutdown().await;
        let status = bridge.wait().await;
        assert!(status.is_some(), "shutdown sonrası süreç çıkmalı");
    }
}