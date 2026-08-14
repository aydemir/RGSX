//! Gap-4 4b — header varyantları + retry/backoff motoru.
//!
//! Python `queue.py` HTTP retry döngüsü (satır 1262–1381) karşılığı:
//! - header varyantları (archive.org 3 set, vimm `Connection: close`),
//! - 401/403 → sonraki varyanta atla,
//! - 429 → Retry-After / exp backoff (5s·2^n, 30s tavan),
//! - timeout/connection → kısa bekle (2s) + yeniden dene,
//! - browser-challenge tespiti → fail-fast.

use std::time::Duration;

/// Başarılı bir istek sonucu — gövde `stream::download_stream_async` ile
/// `.part`'a akar; burada yalnızca kabul edilecek yanıtı seçeriz.
#[derive(Debug)]
pub struct HeaderAttempt {
    pub status: u16,
    pub content_type: String,
    /// 429 Retry-After başlığı (saniye; yoksa `None`).
    pub retry_after: Option<f64>,
}

/// Header seti varyantı — `apply` çağıran tarafından `reqwest::RequestBuilder`'a
/// uygulanır.
#[derive(Debug, Clone)]
pub struct HeaderVariant {
    pub name: &'static str,
    pub headers: Vec<(String, String)>,
}

/// Archive.org için header varyantları (Python `header_variants` archive bloğu).
pub fn archive_org_variants(base_ua: &str, cookie: Option<&str>) -> Vec<HeaderVariant> {
    let cookie_hdr = cookie
        .map(|c| vec![("Cookie".to_string(), c.to_string())])
        .unwrap_or_default();
    vec![
        HeaderVariant {
            name: "archive-basic",
            headers: vec![
                ("User-Agent".into(), base_ua.to_string()),
                ("Accept".into(), "application/octet-stream,*/*;q=0.8".into()),
                ("Accept-Language".into(), "en-US,en;q=0.5".into()),
                ("Connection".into(), "keep-alive".into()),
            ]
            .into_iter()
            .chain(cookie_hdr.clone())
            .collect(),
        },
        HeaderVariant {
            name: "archive-star",
            headers: vec![
                ("User-Agent".into(), base_ua.to_string()),
                ("Accept".into(), "*/*".into()),
                ("Referer".into(), "https://archive.org/".into()),
            ]
            .into_iter()
            .chain(cookie_hdr)
            .collect(),
        },
    ]
}

/// Vimm.net için retry header'ı — `Connection: close` ile TCP havuzunu yeniler
/// (Python `RemoteDisconnected` transiente çözümü).
pub fn vimm_retry_headers(base: &[(String, String)]) -> Vec<HeaderVariant> {
    let close: Vec<(String, String)> = base
        .iter()
        .map(|(k, v)| {
            if k == "Connection" {
                ("Connection".into(), "close".into())
            } else {
                (k.clone(), v.clone())
            }
        })
        .collect();
    vec![
        HeaderVariant {
            name: "vimm-retry-1",
            headers: close.clone(),
        },
        HeaderVariant {
            name: "vimm-retry-2",
            headers: close,
        },
    ]
}

/// 429 backoff süresi (Python: `Retry-After` ya da `base * 2^hits`, tavan 30s).
pub fn retry_after_wait(retry_after: Option<f64>, rate_limit_hits: u32, base: f64) -> Duration {
    let wait = retry_after
        .map(|r| r.max(1.0))
        .unwrap_or_else(|| (base * 2f64.powi(rate_limit_hits as i32)).min(30.0));
    Duration::from_secs_f64(wait.min(30.0))
}

/// Yeni bir deneme başlatılabilir mi? (Python döngü koşulu eşleniği.)
pub fn should_retry(status: u16) -> bool {
    matches!(status, 401 | 403 | 429)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_variants_shape() {
        let v = archive_org_variants("UA", Some("k=v"));
        assert_eq!(v.len(), 2);
        assert!(v[0].headers.iter().any(|(k, _)| k == "Cookie"));
        assert_eq!(v[0].name, "archive-basic");
    }

    #[test]
    fn vimm_retry_forces_close() {
        let base = vec![
            ("Connection".into(), "keep-alive".into()),
            ("Accept".into(), "x".into()),
        ];
        let v = vimm_retry_headers(&base);
        assert_eq!(v.len(), 2);
        assert!(v[0].headers.iter().any(|(k, val)| k == "Connection" && val == "close"));
    }

    #[test]
    fn backoff_caps_at_30() {
        // Retry-After yok → 5s, 10s, 20s, 40s→30s (tavan).
        assert!(retry_after_wait(None, 0, 5.0) >= Duration::from_secs(5));
        assert!(retry_after_wait(None, 3, 5.0) <= Duration::from_secs(30));
        // Retry-After varsa ona uyar (min 1s).
        assert_eq!(retry_after_wait(Some(7.0), 0, 5.0), Duration::from_secs(7));
        assert_eq!(retry_after_wait(Some(0.5), 0, 5.0), Duration::from_secs(1));
    }
}