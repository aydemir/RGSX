//! TASK-012m — manager self-update (Faz 1-4 güvenli iskelet).
//!
//! Faz 5 (binary replace + relaunch) BU turda YOK — ayrı, açık onaylı alt adım.
//! Akış: versiyon manifest'i çek → `CARGO_PKG_VERSION` ile karşılaştır →
//! yeniyse SSE `manager_update` yayınla + `StateData`'ya yaz. TVUI prompt →
//! (kullanıcı onaylayınca) indir + SHA256 doğrula (üzerine YAZMA).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Sender;
use tracing::{info, warn};

use crate::state::StateData;

/// Uzak versiyon manifest'i (`RGSX_UPDATE_MANIFEST_URL` ile sunulur).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

/// `"x.y.z"` → `(major, minor, patch)`. Parse edilemezse `None`.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim_start_matches('v').split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it
        .next()
        .unwrap_or("0")
        .split('-')
        .next()
        .unwrap_or("0")
        .parse()
        .ok()?;
    Some((major, minor, patch))
}

/// `remote` > `current` mı? (basit semver karşılaştırması).
pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

/// Manifest JSON'ı çeker (ağ). Yoksa `None`.
pub async fn fetch_manifest(url: &str) -> Option<VersionInfo> {
    let resp = reqwest::Client::new().get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<VersionInfo>().await.ok()
}

/// Arka plan kontrolü: manifest çek → karşılaştır → yeniyse SSE + StateData.
pub async fn check_update(events: Sender<String>, state_data: Arc<RwLock<StateData>>) {
    let manifest_url = match std::env::var("RGSX_UPDATE_MANIFEST_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return, // Yapılandırılmamışsa parity gereği no-op.
    };
    let current = env!("CARGO_PKG_VERSION");
    let Some(info) = fetch_manifest(&manifest_url).await else {
        warn!("manager self-update: manifest çekilemedi ({manifest_url})");
        return;
    };
    if is_newer(&info.version, current) {
        info!("manager güncellemesi mevcut: {current} → {}", info.version);
        let payload = serde_json::json!({
            "available": true,
            "version": info.version,
            "url": info.url,
            "sha256": info.sha256,
        });
        {
            let mut g = state_data.write().unwrap();
            g.manager_update = Some(payload.clone());
        }
        crate::sse::publish(&events, "manager_update", &payload);
    }
}

/// İndir + SHA256 doğrula (üzerine YAZMA). Başarılıysa indirilen dosya yolu.
/// `expected_sha256` verilmişse uyum zorunlu; uymazsa `Err`.
pub async fn download_and_verify(
    url: &str,
    expected_sha256: Option<&str>,
) -> Result<PathBuf, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("indirme hatası: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("indirme HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("gövde okuma: {e}"))?;
    // SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex_encode(&hasher.finalize());
    if let Some(exp) = expected_sha256 {
        if actual.to_lowercase() != exp.to_lowercase() {
            return Err(format!("SHA256 uyumsuz: beklenen {exp}, gerçek {actual}"));
        }
    }
    // Geçici dosyaya yaz (üzerine yazma YOK — faz 5 ayrı).
    let tmp = std::env::temp_dir().join(format!("rgsx-manager-update-{actual}.bin"));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("dosya yazma: {e}"))?;
    Ok(tmp)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse_version("x.y.z"), None);
    }

    #[test]
    fn is_newer_semver() {
        assert!(is_newer("1.2.4", "1.2.3"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.2", "1.2.3"));
    }
}
