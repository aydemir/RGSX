//! Faz 12e — indirme resolver soyutlaması.
//!
//! Python `one_fichier.py` / `utils/torrent.py` çözüm mantığının Rust karşılığı.
//! Bir kaynak URL'i çözülür: ya torrent (magnet/`.torrent` → librqbit) ya da doğrudan
//! indirilebilir HTTP(S) dosyası (DDL). Debrid servisleri (1Fichier / RealDebrid)
//! kimlik bilgisi (API key) ile etkinleşir; olmadan `NotConfigured` döner ve zincir
//! `DirectResolver`'a (her zaman çözer) düşer.
//!
//! Not: debrid'in gerçek ağ çağrısı (link çözme) bu ortamda test edilemez; `Resolver`
//! uygulamaları kimlik yoksa `NotConfigured`, varsa `NotImplemented` (ağ gerektirir)
//! döndürür. Native DDL fetch (`manager-http` tarafında reqwest ile) doğrudan HTTP
//! kaynakları için çalışır.

pub mod http;

use std::sync::Arc;

/// Çözülmüş indirme kaynağı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadSource {
    /// `magnet:` / `.torrent` — `manager-torrent` (librqbit) tarafından indirilir.
    Torrent(String),
    /// Doğrudan indirilebilir HTTP(S) dosya linki (DDL).
    DirectHttp(String),
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ResolveError {
    #[error("torrent/HTTP kaynağı değil: {0}")]
    NotTorrent(String),
    #[error("resolver yapılandırılmamış: {0}")]
    NotConfigured(String),
    #[error("native fetch henüz uygulanmadı: {0}")]
    NotImplemented(String),
    #[error("çözüm hatası: {0}")]
    Other(String),
}

/// Bir URL'i `DownloadSource`'a çözen soyutlama.
pub trait Resolver: Send + Sync {
    fn name(&self) -> &'static str;
    fn resolve(&self, url: &str) -> Result<DownloadSource, ResolveError>;
}

/// Torrent/magnet veya düz HTTP dosyasını doğrudan sınıflandırır (proxy yok).
pub struct DirectResolver;

impl Resolver for DirectResolver {
    fn name(&self) -> &'static str {
        "direct"
    }
    fn resolve(&self, url: &str) -> Result<DownloadSource, ResolveError> {
        if is_torrent_url(url) {
            return Ok(DownloadSource::Torrent(url.to_string()));
        }
        if url.starts_with("http://") || url.starts_with("https://") {
            return Ok(DownloadSource::DirectHttp(url.to_string()));
        }
        Err(ResolveError::NotTorrent(url.to_string()))
    }
}

/// 1Fichier debrid resolver — `RGSX_1FICHIER_KEY` ile etkinleşir.
pub struct OneFichierResolver {
    key: Option<String>,
}

impl OneFichierResolver {
    pub fn from_env() -> Self {
        Self {
            key: std::env::var("RGSX_1FICHIER_KEY").ok(),
        }
    }
}

impl Resolver for OneFichierResolver {
    fn name(&self) -> &'static str {
        "1fichier"
    }
    fn resolve(&self, _url: &str) -> Result<DownloadSource, ResolveError> {
        match &self.key {
            None => Err(ResolveError::NotConfigured(
                "RGSX_1FICHIER_KEY gerekli".into(),
            )),
            Some(_) => Err(ResolveError::NotImplemented(
                "1Fichier native link çözme ağ gerektirir; sonraki alt görevde".into(),
            )),
        }
    }
}

/// RealDebrid resolver — `RGSX_REALDEBRID_KEY` ile etkinleşir.
pub struct RealDebridResolver {
    key: Option<String>,
}

impl RealDebridResolver {
    pub fn from_env() -> Self {
        Self {
            key: std::env::var("RGSX_REALDEBRID_KEY").ok(),
        }
    }
}

impl Resolver for RealDebridResolver {
    fn name(&self) -> &'static str {
        "realdebrid"
    }
    fn resolve(&self, _url: &str) -> Result<DownloadSource, ResolveError> {
        match &self.key {
            None => Err(ResolveError::NotConfigured(
                "RGSX_REALDEBRID_KEY gerekli".into(),
            )),
            Some(_) => Err(ResolveError::NotImplemented(
                "RealDebrid native link çözme ağ gerektirir; sonraki alt görevde".into(),
            )),
        }
    }
}

/// Resolver zinciri — debrid'ler önce denenir, yapılandırılmamışsa `DirectResolver`'a düşer.
pub struct DownloadManager {
    resolvers: Vec<Arc<dyn Resolver>>,
}

impl DownloadManager {
    pub fn new() -> Self {
        Self {
            resolvers: vec![
                Arc::new(OneFichierResolver::from_env()),
                Arc::new(RealDebridResolver::from_env()),
                Arc::new(DirectResolver),
            ],
        }
    }

    /// İlk başarılı çözümü döner. Debrid'ler yalnız `NotConfigured`/`NotImplemented`
    /// döndürürse zincir `DirectResolver`'a (her zaman çözer) ulaşır; gerçek hata anında döner.
    pub fn resolve(&self, url: &str) -> Result<DownloadSource, ResolveError> {
        let mut last_err = ResolveError::NotTorrent(url.to_string());
        for r in &self.resolvers {
            match r.resolve(url) {
                Ok(s) => return Ok(s),
                Err(e) => match &e {
                    ResolveError::NotConfigured(_) | ResolveError::NotImplemented(_) => {
                        last_err = e;
                    }
                    _ => return Err(e),
                },
            }
        }
        Err(last_err)
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// TASK-002l ile aynı: `magnet:` / `rgsx+torrent:` / `.torrent` torrent şemasıdır.
pub fn is_torrent_url(url: &str) -> bool {
    let u = url.trim().to_ascii_lowercase();
    if u.starts_with("magnet:") || u.starts_with("rgsx+torrent:") {
        return true;
    }
    u.split('?').next().unwrap_or("").ends_with(".torrent")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_resolver_classifies() {
        assert_eq!(
            DirectResolver.resolve("magnet:?xt=urn:btih:abc"),
            Ok(DownloadSource::Torrent("magnet:?xt=urn:btih:abc".into()))
        );
        assert_eq!(
            DirectResolver.resolve("https://x.com/game.zip"),
            Ok(DownloadSource::DirectHttp("https://x.com/game.zip".into()))
        );
        assert!(matches!(
            DirectResolver.resolve("ftp://x/com/file.bin"),
            Err(ResolveError::NotTorrent(_))
        ));
    }

    #[test]
    fn debrid_unconfigured_falls_to_direct() {
        let m = DownloadManager::new();
        assert_eq!(
            m.resolve("https://x.com/game.zip"),
            Ok(DownloadSource::DirectHttp("https://x.com/game.zip".into()))
        );
        assert_eq!(
            m.resolve("magnet:?xt=urn:btih:abc"),
            Ok(DownloadSource::Torrent("magnet:?xt=urn:btih:abc".into()))
        );
    }

    #[test]
    fn onefichier_not_configured_without_key() {
        std::env::remove_var("RGSX_1FICHIER_KEY");
        let r = OneFichierResolver::from_env();
        assert!(matches!(r.resolve("https://1fichier.com/x"), Err(ResolveError::NotConfigured(_))));
    }
}
