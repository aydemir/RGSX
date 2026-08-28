//! TASK-002-gap-11 Faz1 — 1fichier provider zinciri iskeleti.
//!
//! `network/one_fichier.py` OF0..OF18 parity iskeleti:
//! - Provider sıralı fallback 1F→AD→DL→RD→TB→FREE
//! - API key'leri (mtime aware yerine env/file okuma, test edilebilir)
//! - Duplicate URL dedup (≤1800s bekleme + cache)
//! - `provider_used`/`provider_prefix` history alanları

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Provider sıralı fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    OneFichier,
    AllDebrid,
    DebridLink,
    RealDebrid,
    TorBox,
    Free,
}

impl Provider {
    pub fn all_in_order() -> &'static [Provider] {
        &[Provider::OneFichier, Provider::AllDebrid, Provider::DebridLink, Provider::RealDebrid, Provider::TorBox, Provider::Free]
    }
    pub fn prefix(&self) -> &'static str {
        match self {
            Provider::OneFichier => "1F",
            Provider::AllDebrid => "AD",
            Provider::DebridLink => "DL",
            Provider::RealDebrid => "RD",
            Provider::TorBox => "TB",
            Provider::Free => "FREE",
        }
    }
    pub fn display_prefix(&self) -> String {
        if *self == Provider::Free { "".into() } else { format!("{}:", self.prefix()) }
    }
}

/// API key'leri (5 provider).
#[derive(Debug, Clone, Default)]
pub struct ApiKeys {
    pub onefichier: String,
    pub alldebrid: String,
    pub debridlink: String,
    pub realdebrid: String,
    pub torbox: String,
}

impl ApiKeys {
    /// Env'den yükle (test edilebilir). Dosya yolu env `RGSX_API_KEY_*_PATH` varsa oradan, yoksa doğrudan `RGSX_*_KEY`.
    pub fn from_env() -> Self {
        Self {
            onefichier: load_key("RGSX_1FICHIER_KEY", "RGSX_API_KEY_1FICHIER_PATH"),
            alldebrid: load_key("RGSX_ALLDEBRID_KEY", "RGSX_API_KEY_ALLDEBRID_PATH"),
            debridlink: load_key("RGSX_DEBRIDLINK_KEY", "RGSX_API_KEY_DEBRIDLINK_PATH"),
            realdebrid: load_key("RGSX_REALDEBRID_KEY", "RGSX_API_KEY_REALDEBRID_PATH"),
            torbox: load_key("RGSX_TORBOX_KEY", "RGSX_API_KEY_TORBOX_PATH"),
        }
    }
    pub fn has(&self, p: Provider) -> bool {
        match p {
            Provider::OneFichier => !self.onefichier.is_empty(),
            Provider::AllDebrid => !self.alldebrid.is_empty(),
            Provider::DebridLink => !self.debridlink.is_empty(),
            Provider::RealDebrid => !self.realdebrid.is_empty(),
            Provider::TorBox => !self.torbox.is_empty(),
            Provider::Free => true,
        }
    }
    /// Provider fallback sırası: mevcut key'i olanlar + Free (her zaman)
    pub fn available_providers(&self) -> Vec<Provider> {
        Provider::all_in_order().iter().copied().filter(|p| self.has(*p)).collect()
    }
}

fn load_key(env_key: &str, env_path: &str) -> String {
    if let Ok(p) = std::env::var(env_path) {
        if !p.is_empty() {
            if let Ok(s) = std::fs::read_to_string(&p) {
                let t = s.trim().to_string();
                if !t.is_empty() { return t; }
            }
        }
    }
    std::env::var(env_key).unwrap_or_default().trim().to_string()
}

/// Duplicate URL dedup cache (OF1): url -> (provider_used, Instant).
/// ≤1800s içinde aynı URL tekrar istenirse bekle + cache sonucu.
#[derive(Debug, Clone)]
pub struct DedupCache {
    inner: Arc<Mutex<HashMap<String, (String, Instant)>>>,
    ttl: Duration,
}

impl Default for DedupCache {
    fn default() -> Self { Self::new(Duration::from_secs(1800)) }
}

impl DedupCache {
    pub fn new(ttl: Duration) -> Self {
        Self { inner: Arc::new(Mutex::new(HashMap::new())), ttl }
    }
    pub fn get(&self, url: &str) -> Option<String> {
        let map = self.inner.lock().ok()?;
        let (prov, at) = map.get(url)?;
        if at.elapsed() <= self.ttl { Some(prov.clone()) } else { None }
    }
    pub fn insert(&self, url: &str, provider_used: &str) {
        if let Ok(mut m) = self.inner.lock() {
            m.insert(url.to_string(), (provider_used.to_string(), Instant::now()));
        }
    }
    pub fn clear_expired(&self) {
        if let Ok(mut m) = self.inner.lock() {
            m.retain(|_, (_, at)| at.elapsed() <= self.ttl);
        }
    }
}

/// History `provider_used`/`provider_prefix` alanlarını üretir (UI parity: "AD:" gibi).
pub fn history_provider_fields(provider: Provider) -> (String, String) {
    let used = provider.prefix().to_string();
    let prefix = provider.display_prefix();
    (used, prefix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_order_and_prefix() {
        let order = Provider::all_in_order();
        assert_eq!(order[0], Provider::OneFichier);
        assert_eq!(order[5], Provider::Free);
        assert_eq!(Provider::AllDebrid.prefix(), "AD");
        assert_eq!(Provider::Free.display_prefix(), "");
        assert_eq!(Provider::TorBox.display_prefix(), "TB:");
    }

    #[test]
    fn api_keys_available_providers() {
        let keys = ApiKeys { onefichier: "k1".into(), alldebrid: "".into(), debridlink: "k3".into(), realdebrid: "".into(), torbox: "".into() };
        let avail = keys.available_providers();
        assert!(avail.contains(&Provider::OneFichier));
        assert!(avail.contains(&Provider::DebridLink));
        assert!(avail.contains(&Provider::Free));
        assert!(!avail.contains(&Provider::AllDebrid));
    }

    #[test]
    fn dedup_cache_ttl() {
        let c = DedupCache::new(Duration::from_millis(50));
        c.insert("http://x", "1F");
        assert_eq!(c.get("http://x"), Some("1F".into()));
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(c.get("http://x"), None);
        c.insert("http://x", "AD");
        assert_eq!(c.get("http://x"), Some("AD".into()));
    }

    #[test]
    fn history_fields() {
        let (used, prefix) = history_provider_fields(Provider::AllDebrid);
        assert_eq!(used, "AD");
        assert_eq!(prefix, "AD:");
        let (used2, prefix2) = history_provider_fields(Provider::Free);
        assert_eq!(used2, "FREE");
        assert_eq!(prefix2, "");
    }

    #[test]
    fn load_key_from_env() {
        std::env::set_var("RGSX_1FICHIER_KEY", "test123");
        let k = load_key("RGSX_1FICHIER_KEY", "RGSX_API_KEY_1FICHIER_PATH");
        assert_eq!(k, "test123");
        std::env::remove_var("RGSX_1FICHIER_KEY");
    }
}
