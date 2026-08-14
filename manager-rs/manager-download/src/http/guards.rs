//! Gap-4 4a — indirme guard'ları.
//!
//! Python `network/http_download.py` (`_is_browser_challenge_response`,
//! `_looks_like_html_or_challenge`, `_matches_expected_archive_signature`,
//! `_should_accept_partial_archive`) ve `lolroms.py` imza yardımcılarının
//! Rust karşılığı. `HttpDownloader` stream öncesi/sonrası bunları kullanır.

use std::path::Path;

/// Browser-challenge sayfalarını (Cloudflare vb.) hızlı tespit eder (Python
/// `_is_browser_challenge_response` — 403/429/503 + marker). Davranış birebir:
/// durum bu sette değilse `false`.
pub fn is_browser_challenge(status: u16, body_prefix: &[u8]) -> bool {
    if !matches!(status, 403 | 429 | 503) {
        return false;
    }
    let head = String::from_utf8_lossy(&body_prefix[..body_prefix.len().min(4000)])
        .to_ascii_lowercase();
    let markers = [
        "just a moment",
        "cf_chl_opt",
        "challenge-platform",
        "enable javascript and cookies to continue",
        "checking your browser before accessing",
    ];
    markers.iter().any(|m| head.contains(m))
}

/// Dosyanın başı HTML/challenge içeriyor mu? (Python `_looks_like_html_or_challenge`
/// — ilk 2048 byte; tespit edilemezse `true` kabul eder).
pub fn looks_like_html_or_challenge(data: &[u8]) -> bool {
    if data.is_empty() {
        return true;
    }
    let head = data[..data.len().min(2048)].to_ascii_lowercase();
    let markers: &[&[u8]] = &[
        b"<html",
        b"<!doctype html",
        b"cloudflare",
        b"just a moment",
        b"cf-chl",
        b"challenge-platform",
    ];
    markers.iter().any(|m| {
        head
            .windows(m.len())
            .any(|w| w.eq_ignore_ascii_case(m))
    })
}

/// Arşiv uzantıları (.7z/.zip/.rar) için dosya imza kontrolü (Python
/// `_matches_expected_archive_signature`). Arşiv olmayan payload `true`.
pub fn matches_expected_archive_signature(file_path: &Path, head: &[u8]) -> bool {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "7z" => head.starts_with(&[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]),
        "zip" => {
            head.starts_with(b"PK\x03\x04")
                || head.starts_with(b"PK\x05\x06")
                || head.starts_with(b"PK\x07\x08")
        }
        "rar" => {
            head.starts_with(b"Rar!\x1a\x07\x00")
                || head.starts_with(b"Rar!\x1a\x07\x01\x00")
        }
        _ => true,
    }
}

/// Kısmi arşiv kabul kuralı (Python `_should_accept_partial_archive`). Dönen
/// `(kabul, neden)`.
pub fn should_accept_partial_archive(
    downloaded: u64,
    total_size: u64,
    file_path: &Path,
    head: &[u8],
) -> (bool, &'static str) {
    if total_size == 0 || downloaded >= total_size {
        return (true, "archive complete");
    }
    let difference = total_size.saturating_sub(downloaded);
    if difference == 0 {
        return (true, "archive complete");
    }
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "7z" | "zip" | "rar") {
        return (true, "non-archive payload");
    }
    if !matches_expected_archive_signature(file_path, head) {
        return (false, "invalid archive signature");
    }
    if difference <= 16 {
        return (true, "small size mismatch tolerated");
    }
    let ratio = difference as f64 / total_size as f64;
    if ratio <= 0.0005 && difference <= 64 {
        return (true, "tiny size mismatch tolerated");
    }
    if ext == "zip" && zip_validates_central_directory(file_path) {
        return (true, "archive validates despite partial size mismatch");
    }
    (false, "incomplete archive payload downloaded")
}

/// ZIP merkez dizinini doğrular (Python `zipfile.testzip()` eşleniği — bozuk
/// başlık ya da eksik EOCD'yi yakalar). Sadece uzaklık/geçerlilik kontrolü;
/// içerik çıkarmaz.
fn zip_validates_central_directory(file_path: &Path) -> bool {
    use std::io::{Read, Seek};
    let file = match std::fs::File::open(file_path) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let size = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => return false,
    };
    if size < 22 {
        return false;
    }
    let mut reader = file;
    // Son 64KB + EOCD tarama penceresi.
    let window = size.min(65536) as usize;
    let mut tail = vec![0u8; window];
    if reader.seek(std::io::SeekFrom::End(-(window as i64))).is_err() {
        return false;
    }
    if reader.read_exact(&mut tail).is_err() {
        return false;
    }
    // EOCD imzası: PK\x05\x06.
    tail.windows(4)
        .any(|w| w == b"PK\x05\x06")
}

/// Content-Range başlığından toplam boyut ('bytes 0-99/1000' -> 1000). Python
/// `_http_parse_content_range` karşılığı.
pub fn parse_content_range_total(header: &str) -> Option<u64> {
    let h = header.trim().to_ascii_lowercase();
    let rest = h.strip_prefix("bytes ")?;
    let (_, total) = rest.split_once('/')?;
    let total = total.trim();
    if total == "*" {
        return None;
    }
    total.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_challenge_detection() {
        let body = b"<html>Just a moment... checking your browser</html>";
        assert!(is_browser_challenge(403, body));
        assert!(is_browser_challenge(503, body));
        assert!(!is_browser_challenge(200, body));
        assert!(!is_browser_challenge(403, b"<html>normal error</html>"));
    }

    #[test]
    fn html_or_challenge_markers() {
        assert!(looks_like_html_or_challenge(b"<html><body>"));
        assert!(looks_like_html_or_challenge(b"<!DOCTYPE html>"));
        assert!(looks_like_html_or_challenge(b"Cloudflare challenge-platform"));
        assert!(!looks_like_html_or_challenge(b"PK\x03\x04binary"));
    }

    #[test]
    fn archive_signatures() {
        assert!(matches_expected_archive_signature(
            Path::new("x.zip"),
            b"PK\x03\x04abc"
        ));
        assert!(!matches_expected_archive_signature(
            Path::new("x.zip"),
            b"<html>"
        ));
        assert!(matches_expected_archive_signature(
            Path::new("x.7z"),
            &[0x37, 0x7a, 0xbc, 0xaf, 0x27, 0x1c]
        ));
        assert!(matches_expected_archive_signature(
            Path::new("x.bin"),
            b"whatever"
        ));
    }

    #[test]
    fn partial_archive_acceptance() {
        let zip = Path::new("x.zip");
        // Tamam → kabul.
        let (ok, _) = should_accept_partial_archive(1000, 1000, zip, b"PK\x03\x04");
        assert!(ok);
        // Non-archive payload → kabul.
        let (ok, _) = should_accept_partial_archive(100, 1000, Path::new("x.bin"), b"data");
        assert!(ok);
        // Kısmi arşiv, kötü imza → reddet.
        let (ok, reason) = should_accept_partial_archive(100, 1000, zip, b"<html>");
        assert!(!ok);
        assert_eq!(reason, "invalid archive signature");
        // Küçük fark (<=16) → kabul.
        let (ok, _) = should_accept_partial_archive(984, 1000, zip, b"PK\x03\x04");
        assert!(ok);
        // Büyük fark → reddet.
        let (ok, reason) = should_accept_partial_archive(500, 1000, zip, b"PK\x03\x04");
        assert!(!ok);
        assert_eq!(reason, "incomplete archive payload downloaded");
    }

    #[test]
    fn content_range_parse() {
        assert_eq!(parse_content_range_total("bytes 0-99/1000"), Some(1000));
        assert_eq!(parse_content_range_total("bytes 200-300/*"), None);
        assert_eq!(parse_content_range_total("nope"), None);
    }
}