//! Gap-4 — native HTTP-doğrudan indirme motoru (`manager-download` içinde).
//!
//! Python `network/queue.py` HTTP alt-ağacı (H0..H12) + `network/http_download.py` /
//! `network/archive_org.py` / `network/lolroms.py` yardımcılarının Rust karşılığı.
//!
//! ## Modüller
//! - `guards` — browser-challenge/HTML tespiti, arşiv imza + kısmi kabul.
//! - `stream` — `.part`'a stream yazma, Range resume, progress, cancel.
//! - `headers` — header varyantları + retry/backoff (faz 4b).
//! - `vimm` — vimm.net form/mediaId çözümü (faz 4c).
//! - `archive_org` — archive.org cookie/metadata/alt-URL (faz 4d).
//! - `lolroms` — lolroms.com parent-warm + indirme (faz 4f, reqwest fallback).
//!
//! ## Faz takibi
//! - 4a ✅ stream çekirdek (bu dosya + stream.rs + guards.rs)
//! - 4b ✅ header varyantları + 429/retry (headers.rs)
//! - 4c ✅ vimm.net
//! - 4d ✅ archive.org
//! - 4e ✅ rust_daemon/WebUI delegasyonu
//! - 4f ✅ lolroms reqwest fallback (parent GET warm + Referer + guards)

pub mod archive_org;
pub mod guards;
pub mod headers;
pub mod lolroms;
pub mod stream;
pub mod vimm;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use self::guards::looks_like_html_or_challenge;
use self::headers::HeaderVariant;
use self::stream::{CancelFlag, ProgressCb};

/// İndirme hatası — Python `requests.HTTPError` / `InsufficientDiskSpaceError`
/// karşılığı. `kind` akış yukarı (api.rs / rust_daemon) için sınıflandırma sağlar.
#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("{0}")]
    Http(String),
    #[error("browser challenge tespit edildi — etkileşimli tarayıcı gerekli")]
    BrowserChallenge,
    #[error("HTML/challenge içerik arşiv yerine indirildi: {0}")]
    HtmlInsteadOfPayload(String),
    #[error("indirilen payload geçerli bir arşiv değil")]
    InvalidArchive,
    #[error("arşiv kısmi kabul edilmedi: {0}")]
    PartialArchiveRejected(String),
    #[error("kaynak boş yanıt döndü: {0}")]
    EmptyResponse(String),
    #[error("disk alanı yetersiz: {0}")]
    InsufficientDiskSpace(String),
    #[error("yazma izni yok: {0}")]
    PermissionDenied(String),
    #[error("indirme iptal edildi")]
    Canceled,
    #[error("istemci hatası: {0}")]
    Client(String),
    #[error("ağ hatası: {0}")]
    Network(String),
}

impl DownloadError {
    /// Feature-flag yolu için okunabilir mesaj.
    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl From<std::io::Error> for DownloadError {
    fn from(e: std::io::Error) -> Self {
        DownloadError::Client(e.to_string())
    }
}

impl From<reqwest::Error> for DownloadError {
    fn from(e: reqwest::Error) -> Self {
        // TASK-002-gap-32: bağlantı/gönderme hataları AĞ hatası sayılmalı ki
        // ardışık hata sayacı (`network_error_streak`) artsın ve `network_down`
        // bayrağı set edilsin. `is_connect()` yalnız "Connect" kind'i yakalar;
        // ama "error sending request for url" gibi hatalar `Request` kind'i
        // altında gelir (içinde ConnectionRefused olmasına rağmen) ve
        // `is_connect()` false döner → `DownloadError::Http`'a düşüp streak
        // artmazdı. Bu da gerçek WiFi kesintisinde banner'ın tetiklenmemesine
        // yol açardı. Bu yüzden `is_request()` + mesaj tabanlı kontrol de ekli.
        if e.is_timeout() || e.is_connect() || e.is_request() {
            return DownloadError::Network(format!("ağ: {e}"));
        }
        let msg = e.to_string().to_ascii_lowercase();
        if msg.contains("connection")
            || msg.contains("refused")
            || msg.contains("resolve")
            || msg.contains("dns")
            || msg.contains("sending request")
            || msg.contains("timed out")
            || msg.contains("no route")
            || msg.contains("host")
        {
            return DownloadError::Network(format!("ağ: {e}"));
        }
        DownloadError::Http(e.to_string())
    }
}

/// Varsayılan browser benzeri header'lar (Python `_build_browser_download_headers`).
pub fn default_browser_headers(referer: Option<&str>) -> Vec<(String, String)> {
    let mut h = vec![
        (
            "User-Agent".into(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/122.0.0.0 Safari/537.36".into(),
        ),
        ("Accept".into(), "application/octet-stream,*/*;q=0.8".into()),
        ("Accept-Language".into(), "en-US,en;q=0.9,fr;q=0.8".into()),
        ("Accept-Encoding".into(), "identity".into()),
        ("Connection".into(), "keep-alive".into()),
        ("DNT".into(), "1".into()),
    ];
    if let Some(r) = referer {
        h.push(("Referer".into(), r.to_string()));
    }
    h
}

/// İndirme isteği yapılandırması (Python `download_rom` HTTP dalı parametreleri).
#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub dest_path: PathBuf,
    /// Bilinen uzak boyut (vimm sayfa ipucu, torrent `size_bytes` vb.) — fallback.
    pub known_total_size: u64,
    /// Referer override (vimm orijinal sayfa vb.).
    pub referer: Option<String>,
    /// archive.org için cookie (Python `load_archive_org_cookie` eşleniği).
    pub cookie: Option<String>,
}

/// `HttpDownloader` — tek seferlik senkron/async HTTP indirme (faz 4a çekirdeği).
///
/// 4b ile retry/header-varyant döngüsü, 4c/4d provider çözümü bu struct'ın içine
/// entegre edilecek; şimdilik tek istek + guards sıralaması yapar.
#[derive(Clone)]
pub struct HttpDownloader {
    client: Option<reqwest::Client>,
    cancel: Option<Arc<CancelFlag>>,
    on_progress: Option<Arc<ProgressCb>>,
    /// Maksimum deneme sayısı (retry döngüsü, 4b).
    max_retries: u32,
    /// 429 Retry-After yoksa kullanılan taban backoff (saniye); `base * 2^hits`.
    base_backoff: Duration,
}

impl Default for HttpDownloader {
    fn default() -> Self {
        Self {
            client: None,
            cancel: None,
            on_progress: None,
            max_retries: 5,
            base_backoff: Duration::from_secs(5),
        }
    }
}

impl HttpDownloader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Paylaşılan HTTP istemcisi (bağlantı havuzu + cookie oturumu). Python
    /// `requests.Session` eşleniği.
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = Some(client);
        self
    }

    pub fn with_cancel(mut self, cancel: CancelFlag) -> Self {
        self.cancel = Some(Arc::new(cancel));
        self
    }

    pub fn with_progress(mut self, cb: impl Fn(u64, u64) + Send + Sync + 'static) -> Self {
        self.on_progress = Some(Arc::new(cb));
        self
    }

    /// Retry davranışını yapılandırır (4b). `base_backoff` Retry-After yoksa
    /// `base * 2^hits` (tavan 30s) olarak kullanılır.
    pub fn with_retry(mut self, max_retries: u32, base_backoff: Duration) -> Self {
        self.max_retries = max_retries.max(1);
        self.base_backoff = base_backoff;
        self
    }

    fn client(&self) -> reqwest::Client {
        self.client.clone().unwrap_or_else(|| {
            // Python `requests.Session` gibi cookie oturumu — LOLROMs parent fetch
            // (4f) cookie jar'ı ısıtsın, böylece dosya isteği aynı oturumu kullanır.
            reqwest::Client::builder()
                .cookie_store(true)
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
    }

    fn cancel(&self) -> Option<Arc<CancelFlag>> {
        self.cancel.clone()
    }

    /// Senkron (bloklayan) indirme — `tokio::runtime` içinde çalıştırır.
    /// Dönen yol: başarıda nihai `dest_path`.
    pub fn download_blocking(&self, req: &DownloadRequest) -> Result<PathBuf, DownloadError> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| DownloadError::Client(format!("runtime: {e}")))?;
        rt.block_on(self.download_async(req))
    }

    /// Header varyant listesi (4b) — provider'a göre genişlet:
    /// archive.org → 2 varyant, vimm.net → `Connection: close` retry çifti, diğer → tek.
    /// Her varyant TAM header setidir (default ile birleştirilmez → çift UA olmaz).
    fn header_variants(
        &self,
        req: &DownloadRequest,
        referer: Option<&str>,
        cookie: Option<&str>,
    ) -> Vec<HeaderVariant> {
        let base = default_browser_headers(referer);
        let ua = base
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("User-Agent"))
            .map(|(_, v)| v.clone())
            .unwrap_or_else(|| "Mozilla/5.0".into());
        let url_lc = req.url.to_lowercase();
        if url_lc.contains("archive.org") {
            self::headers::archive_org_variants(&ua, cookie)
        } else if url_lc.contains("vimm") {
            self::headers::vimm_retry_headers(&base)
        } else {
            vec![HeaderVariant {
                name: "default",
                headers: base,
            }]
        }
    }

    /// Async indirme — retry döngüsü (4b) + `.part` stream + guards + finalize.
    pub async fn download_async(&self, req: &DownloadRequest) -> Result<PathBuf, DownloadError> {
        // Gap-5 (A+B): indirme başı disk alanı + yazılabilirlik ön-kontrolü.
        // QueryFailed → atla (devam et), diğerleri anlamlı hata (Retry'a gitmez, bakınız
        // `classify_download_error`).
        let dest_dir = req.dest_path.parent().unwrap_or(&req.dest_path);
        match manager_core::disk::precheck_destination(dest_dir, req.known_total_size) {
            Ok(()) => {}
            Err(manager_core::disk::DiskError::QueryFailed(_)) => {}
            Err(manager_core::disk::DiskError::PermissionDenied(m)) => {
                return Err(DownloadError::PermissionDenied(m))
            }
            Err(manager_core::disk::DiskError::InsufficientSpace { free, required }) => {
                return Err(DownloadError::InsufficientDiskSpace(format!(
                    "gerekli {required} bayt, mevcut {free} bayt"
                )))
            }
        }

        let cancel = self.cancel();
        let resume = self::stream::resume_offset(&req.dest_path);
        let base_backoff = self.base_backoff.as_secs_f64();
        let mut variant_idx = 0usize;
        let mut rate_limit_hits = 0u32;
        let mut attempt: u32 = 0;

        // Provider çözümü (4c): vimm.net sayfası → gerçek indirme URL'si + referer.
        let mut effective_url = req.url.clone();
        let mut resolved_referer = req.referer.clone();
        if req.url.to_lowercase().contains("vimm.net") {
            if let Some(info) = self::vimm::fetch_vimm_download_info(&self.client(), &req.url).await
            {
                effective_url = info.download_url.clone();
                resolved_referer = Some(req.url.clone());
            }
        }

        // Provider çözümü (4f): lolroms.com → normalize + parent sayfa GET (cookie/
        // referer ısınması), sonra dosya isteği `Referer: parent_url` ile.
        if self::lolroms::is_lolroms_url(&req.url) {
            effective_url = self::lolroms::normalize_lolroms_url(&req.url);
            let parent = self::lolroms::parent_url(&effective_url);
            let pheaders = self::lolroms::lolroms_headers("https://lolroms.com/");
            // Parent sayfayı GET et (cookie jar ısınması) — sonucu yok say (best-effort).
            let mut pbuilder = self.client().get(&parent);
            for (k, v) in &pheaders {
                pbuilder = pbuilder.header(k, v);
            }
            let _ = pbuilder.send().await;
            resolved_referer = Some(parent.clone());
        }

        // archive.org cookie (4d): request'ten ya da dosyadan.
        let archive_cookie = req
            .cookie
            .clone()
            .or_else(|| self::archive_org::load_archive_org_cookie());

        // archive.org alt-URL'leri (4d): metadata → view_archive.php fallback.
        let mut alt_urls: Vec<String> = Vec::new();
        if req.url.to_lowercase().contains("archive.org/download/") {
            let m_t0 = std::time::Instant::now();
            eprintln!("[TRACE-MD] archive metadata fetch start {}", req.url);
            // TASK-002-gap-32 izleme: archive.org metadata API'sı anonim IP'lerde
            // 5-44s arası yavaş/rate-limit oluyor ve `alt_urls` neredeyse hep 0.
            // Bu yüzden fetch'i best-effort + kısa timeout (3s) ile sarıyoruz:
            // responsive ise 403 fallback'i korunur, yavaşsa atlanır (≤3s gecikme).
            let meta = tokio::time::timeout(
                Duration::from_secs(3),
                self::archive_org::fetch_archive_metadata(
                    &self.client(),
                    &req.url,
                    archive_cookie.as_deref(),
                ),
            )
            .await;
            let timed_out = meta.is_err();
            if let Ok(Some(meta)) = &meta {
                alt_urls = self::archive_org::build_alt_urls(&req.url, meta);
            }
            eprintln!(
                "[TRACE-MD] archive metadata fetch done in {}ms (alt_urls={}, timed_out={})",
                m_t0.elapsed().as_millis(),
                alt_urls.len(),
                timed_out
            );
        }

        let variants =
            self.header_variants(req, resolved_referer.as_deref(), archive_cookie.as_deref());

        loop {
            attempt += 1;
            if cancel.as_ref().map(|c| c.is_set()).unwrap_or(false) {
                return Err(DownloadError::Canceled);
            }

            let mut builder = self.client().get(&effective_url);
            for (k, v) in &variants[variant_idx].headers {
                builder = builder.header(k, v);
            }
            if resume > 0 {
                builder = builder.header("Range", format!("bytes={resume}-"));
            }

            let s_t0 = std::time::Instant::now();
            eprintln!("[TRACE-MD] GET {}", effective_url);
            let resp = match builder.send().await {
                Ok(r) => {
                    eprintln!(
                        "[TRACE-MD] GET status={} in {}ms",
                        r.status().as_u16(),
                        s_t0.elapsed().as_millis()
                    );
                    r
                }
                Err(e) => {
                    eprintln!(
                        "[TRACE-MD] GET error after {}ms: {}",
                        s_t0.elapsed().as_millis(),
                        e
                    );
                    // timeout/connection → kısa bekle + yeniden dene (Python transiente).
                    if attempt >= self.max_retries {
                        return Err(DownloadError::from(e));
                    }
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
            let status = resp.status().as_u16();

            if status == 401 {
                return Err(DownloadError::Http("HTTP 401".into()));
            }
            if status == 429 {
                // Rate-limit → Retry-After / exp backoff, sonra yeniden dene.
                let retry_after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.trim().parse::<f64>().ok());
                if attempt >= self.max_retries {
                    return Err(DownloadError::Http(format!(
                        "HTTP 429 (rate-limit, {rate_limit_hits} hits)"
                    )));
                }
                let wait =
                    self::headers::retry_after_wait(retry_after, rate_limit_hits, base_backoff);
                rate_limit_hits += 1;
                tokio::time::sleep(wait).await;
                continue;
            }
            if (500..=599).contains(&status) {
                // Transient sunucu hatası → kısa bekle + yeniden dene.
                if attempt >= self.max_retries {
                    return Err(DownloadError::Http(format!("HTTP {status}")));
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            if status == 403 {
                // Challenge sayfaları küçük (HTML); tamamını belleğe almak güvenli.
                let body = resp.bytes().await.unwrap_or_default();
                if self::guards::is_browser_challenge(403, &body) {
                    return Err(DownloadError::BrowserChallenge);
                }
                // Değilse → sonraki header variant'a geç (varsa).
                if variant_idx + 1 < variants.len() {
                    variant_idx += 1;
                    continue;
                }
                // Sonraki alt-URL'e geç (4d: archive.org view_archive.php fallback).
                if let Some(alt) = alt_urls.pop() {
                    effective_url = alt;
                    variant_idx = 0;
                    continue;
                }
                return Err(DownloadError::Http("HTTP 403".into()));
            }
            if !(200..=299).contains(&status) && status != 206 {
                return Err(DownloadError::Http(format!("HTTP {status}")));
            }

            // 200/206 → content-type HTML kontrolü (vimm).
            let content_type = resp
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_lowercase());
            if let Some(ct) = &content_type {
                if ct.contains("text/html") {
                    return Err(DownloadError::HtmlInsteadOfPayload(ct.clone()));
                }
            }

            let (s, detect) = self::stream::download_stream_async(
                resp,
                &req.dest_path,
                resume,
                cancel.as_deref(),
                self.on_progress.clone(),
            )
            .await?;

            // Guards sıralaması (Python H10): arşiv ise HTML/challenge + imza + kısmi kabul.
            let ext = req
                .dest_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_ascii_lowercase())
                .unwrap_or_default();
            let is_archive = matches!(ext.as_str(), "7z" | "zip" | "rar");
            if is_archive && s.downloaded > 0 && !detect.is_empty() {
                if looks_like_html_or_challenge(&detect) {
                    let _ = tokio::fs::remove_file(&req.dest_path).await;
                    return Err(DownloadError::HtmlInsteadOfPayload(
                        "archive yerine HTML/challenge".into(),
                    ));
                }
                if !self::guards::matches_expected_archive_signature(&req.dest_path, &detect) {
                    let _ = tokio::fs::remove_file(&req.dest_path).await;
                    return Err(DownloadError::InvalidArchive);
                }
                let (accepted, reason) = self::guards::should_accept_partial_archive(
                    s.downloaded,
                    s.total_size,
                    &req.dest_path,
                    &detect,
                );
                if !accepted {
                    let _ = tokio::fs::remove_file(&req.dest_path).await;
                    return Err(DownloadError::PartialArchiveRejected(reason.to_string()));
                }
            }

            if s.canceled {
                let _ = tokio::fs::remove_file(&req.dest_path).await;
                return Err(DownloadError::Canceled);
            }
            if s.downloaded == 0 {
                return Err(DownloadError::EmptyResponse("0 byte".into()));
            }

            self::stream::finalize_part(&req.dest_path, s.downloaded).await?;
            return Ok(req.dest_path.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_headers_shape() {
        let h = default_browser_headers(Some("https://x.com/"));
        let keys: Vec<&str> = h.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"User-Agent"));
        assert!(keys.contains(&"Referer"));
        let no_ref = default_browser_headers(None);
        assert!(!no_ref.iter().any(|(k, _)| k == "Referer"));
    }
}
