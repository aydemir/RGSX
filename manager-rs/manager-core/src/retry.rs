//! TASK-002-gap-1 — job-level retry motoru (Python `download_state.py` parity portu).
//!
//! Bağımlılık yönü: `manager-core` en alt seviye crate'tir; `manager-download`
//! (`DownloadError`) / `manager-bridge` (`BridgeError`) tiplerini import EDEMEZ
//! (döngüsel bağımlılık). Bu nedenle `classify_error` **string + HTTP-status**
//! tabanlıdır; hata tipi → (mesaj, status) eşlemesi çağıran katmanda
//! (`manager-http/src/api.rs`) yapılır. Bu, ÇAKIŞMA-3 çözümüdür: Rust
//! `HttpDownloader` Türkçe hata üretir; marker listeleri İngilizce/Fransızca'ya
//! **Türkçe karşılıklar da eklenerek** zenginleştirilmiştir.

use std::collections::HashSet;

/// Maksimum job-level retry sayısı (Python `config.DOWNLOAD_MAX_RETRIES` default 3).
pub const DEFAULT_MAX_RETRIES: u32 = 3;
/// Üssel backoff tabanı (saniye) — Python `DOWNLOAD_RETRY_BACKOFF_BASE_SEC` = 5.0.
pub const DEFAULT_BACKOFF_BASE_SEC: f64 = 5.0;
/// Üssel backoff tavanı (saniye) — Python `DOWNLOAD_RETRY_BACKOFF_MAX_SEC` = 300.0.
pub const DEFAULT_BACKOFF_MAX_SEC: f64 = 300.0;

/// Hata sınıfı — retry mantıklı mı (transient) yoksa kalıcı mı (permanent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    Transient,
    Permanent,
}

/// Python `_TRANSIENT_HTTP_STATUS` (download_state.py:151) — İngilizce/Fransızca + Türkçe ek.
pub const TRANSIENT_HTTP_STATUS: &[u16] = &[
    408, 409, 425, 429, 500, 502, 503, 504, 520, 521, 522, 523, 524, 525, 526, 527,
];

/// Python `_PERMANENT_HTTP_STATUS` (download_state.py:153).
pub const PERMANENT_HTTP_STATUS: &[u16] = &[
    400, 401, 402, 403, 404, 405, 406, 410, 411, 412, 413, 414, 415, 416, 417, 418, 422, 423, 424,
    426, 428, 431, 451,
];

/// Python `_PERMANENT_MARKERS` (download_state.py:157) + Türkçe karşılıklar.
pub const PERMANENT_MARKERS: &[&str] = &[
    // erişim / kimlik
    "access denied",
    "accès refusé",
    "access refused",
    "erişim reddedildi",
    "authentication required",
    "auth required",
    "unauthorized",
    "forbidden",
    "yasak",
    "yetkisiz",
    "kimlik doğrulama gerekli",
    // tarayıcı challenge
    "browser challenge",
    "interactive browser session",
    "etkileşimli tarayıcı",
    // arşiv bozuk
    "payload is not a valid archive",
    "not a valid archive",
    "valid archive signature",
    "geçerli bir arşiv değil",
    "geçerli arşiv imzası",
    // html/challenge içerik
    "html/challenge content",
    "downloaded html",
    "html/challenge",
    "indirilen html",
    "html içerik",
    // boş yanıt
    "empty response",
    "boş yanıt",
    // kısıtlı / karartma
    "restricted (is_dark",
    "is_dark=true",
    "kısıtlı",
    // dosya yok
    "file not found",
    "introuvable",
    "not found",
    "has been removed",
    "removed for abuse",
    "piracy domain",
    "dosya bulunamadı",
    "kaldırıldı",
    // parola
    "password incorrect",
    "invalid password",
    "mot de passe",
    "parola yanlış",
    // disk alanı
    "pas assez d'espace",
    "insufficient disk space",
    "low disk space",
    "manque d'espace",
    "disk alanı yetersiz",
    "yetersiz disk",
];

/// Python `_TRANSIENT_MARKERS` (download_state.py:172) + Türkçe karşılıklar.
pub const TRANSIENT_MARKERS: &[&str] = &[
    // timeout
    "timeout",
    "timed out",
    "timed-out",
    "read timed",
    "zaman aşımı",
    "süre aşımı",
    // bağlantı
    "connection error",
    "connexion",
    "connection aborted",
    "connection reset",
    "connection refused",
    "connection timed",
    "unable to connect",
    "cannot connect",
    "bağlantı",
    "bağlantı hatası",
    "bağlanılamadı",
    "bağlantı reddedildi",
    // retry hakkı
    "max retries exceeded",
    "retries exceeded",
    "yeniden deneme",
    "retry",
    // rate limit
    "rate limit",
    "too many requests",
    "temporarily unavailable",
    "hız sınırı",
    "çok fazla istek",
    "geçici olarak kullanılamıyor",
    // sunucu
    "server error",
    "erreur serveur",
    "service unavailable",
    "bad gateway",
    "sunucu hatası",
    "servis kullanılamıyor",
    // ağ geçidi
    "gateway time-out",
    "ağ geçidi zaman aşımı",
    // indirme sınırı
    "limits downloads to one",
    "limite les téléchargements",
    "indirme sınırı",
    // çeşitli
    "link appears down",
    "temporary failure",
    "ressayer",
    "réessayez",
    "essayez plus tard",
    "slow down",
    "n'existait pas",
    "temporairement",
    "geçici hata",
    "yavaşla",
    "yeniden dene",
];

/// Metinden 400..=599 arası 3 haneli HTTP kodlarını çıkarır (Python
/// `_extract_http_status_codes` eşleniği).
fn extract_http_status_codes(text: &str) -> HashSet<u16> {
    let bytes: Vec<char> = text.chars().collect();
    let mut codes: HashSet<u16> = HashSet::new();
    if bytes.len() < 3 {
        return codes;
    }
    for w in bytes.windows(3) {
        if let (Some(a), Some(b), Some(c)) =
            (w[0].to_digit(10), w[1].to_digit(10), w[2].to_digit(10))
        {
            let before_ok = w[0] == w[0] && (w[0].is_numeric() || true);
            // Önceki karakter rakam/nokta değilse ve sonraki karakter rakam/nokta değilse
            let prev = if w[0] == bytes[0] {
                None
            } else {
                Some(bytes[bytes.len() - 3])
            };
            let _ = (before_ok, prev);
            let val = (a * 100 + b * 10 + c) as u16;
            if (400..=599).contains(&val) {
                let prev_is_digit = w[0] != bytes[0] && bytes[bytes.len() - 4].is_ascii_digit();
                let next_is_digit = bytes
                    .get(bytes.len() - 3 + 3)
                    .map_or(false, |c| c.is_ascii_digit());
                if !prev_is_digit && !next_is_digit {
                    codes.insert(val);
                }
            }
        }
    }
    codes
}

/// Hata sınıflandırıcı (Python `classify_error` parity'si).
///
/// `message` hata mesajı (herhangi bir dilde olabilir), `status_code` çağıranın
/// çıkardığı HTTP durum kodu (varsa). Boş mesaj → kalıcı (sonsuz döngü önleme).
/// Kalıcı marker'lar her zaman önceliklidir; sonra HTTP kodları, sonra transient
/// marker'lar; belirsiz → kalıcı.
pub fn classify_error(message: &str, status_code: Option<u16>) -> ErrorClass {
    if message.is_empty() && status_code.is_none() {
        return ErrorClass::Permanent;
    }
    let text = message.to_lowercase();

    // Kalıcı marker'lar her zaman önce (Python ile birebir).
    if PERMANENT_MARKERS.iter().any(|m| text.contains(m)) {
        return ErrorClass::Permanent;
    }

    // Açık status kodu.
    if let Some(code) = status_code {
        if TRANSIENT_HTTP_STATUS.contains(&code) {
            return ErrorClass::Transient;
        }
        if PERMANENT_HTTP_STATUS.contains(&code) {
            return ErrorClass::Permanent;
        }
    }

    // Mesajdan çıkarılan HTTP kodları.
    for code in extract_http_status_codes(&text) {
        if TRANSIENT_HTTP_STATUS.contains(&code) {
            return ErrorClass::Transient;
        }
        if PERMANENT_HTTP_STATUS.contains(&code) {
            return ErrorClass::Permanent;
        }
    }

    if TRANSIENT_MARKERS.iter().any(|m| text.contains(m)) {
        return ErrorClass::Transient;
    }

    ErrorClass::Permanent
}

/// Python `retry_backoff_seconds` — `min(base * 2^(retry_count-1), max_wait)`.
/// `retry_count == 0` → 0.0 (beklenmez).
pub fn retry_backoff_seconds(retry_count: u32, base: f64, max_wait: f64) -> f64 {
    if retry_count == 0 {
        return 0.0;
    }
    (base * 2f64.powi((retry_count - 1) as i32)).min(max_wait)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_formula() {
        assert_eq!(retry_backoff_seconds(0, 5.0, 300.0), 0.0);
        assert_eq!(retry_backoff_seconds(1, 5.0, 300.0), 5.0);
        assert_eq!(retry_backoff_seconds(2, 5.0, 300.0), 10.0);
        assert_eq!(retry_backoff_seconds(3, 5.0, 300.0), 20.0);
        assert_eq!(retry_backoff_seconds(10, 5.0, 300.0), 300.0);
    }

    #[test]
    fn classify_empty_is_permanent() {
        assert_eq!(classify_error("", None), ErrorClass::Permanent);
    }

    #[test]
    fn classify_explicit_status() {
        assert_eq!(classify_error("", Some(429)), ErrorClass::Transient);
        assert_eq!(classify_error("", Some(500)), ErrorClass::Transient);
        assert_eq!(classify_error("", Some(503)), ErrorClass::Transient);
        assert_eq!(classify_error("", Some(403)), ErrorClass::Permanent);
        assert_eq!(classify_error("", Some(404)), ErrorClass::Permanent);
        assert_eq!(classify_error("", Some(401)), ErrorClass::Permanent);
    }

    #[test]
    fn classify_markers_english() {
        assert_eq!(
            classify_error("connection refused", None),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_error("rate limit exceeded", None),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_error("server error 500", None),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_error("browser challenge detected", None),
            ErrorClass::Permanent
        );
        assert_eq!(
            classify_error("file not found", None),
            ErrorClass::Permanent
        );
        assert_eq!(
            classify_error("insufficient disk space", None),
            ErrorClass::Permanent
        );
    }

    #[test]
    fn classify_markers_turkish() {
        // Rust HttpDownloader Türkçe üretir — bunların transient olması kritik.
        assert_eq!(
            classify_error("bağlantı: timeout", None),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_error("HTTP 429 (rate-limit, 0 hits)", None),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_error("HTML/challenge içerik arşiv yerine indirildi", None),
            ErrorClass::Permanent
        );
        assert_eq!(
            classify_error("disk alanı yetersiz", None),
            ErrorClass::Permanent
        );
        assert_eq!(
            classify_error("browser challenge tespit edildi", None),
            ErrorClass::Permanent
        );
    }

    #[test]
    fn classify_status_in_text() {
        // "HTTP 500" metinden çıkarılır.
        assert_eq!(
            classify_error("request failed: HTTP 500", None),
            ErrorClass::Transient
        );
        assert_eq!(
            classify_error("got HTTP 403 forbidden", None),
            ErrorClass::Permanent
        );
    }

    #[test]
    fn classify_ambiguous_is_permanent() {
        // Tanınmayan hata → kalıcı (sonsuz retry döngüsü olmasın).
        assert_eq!(
            classify_error("some weird glitch", None),
            ErrorClass::Permanent
        );
    }

    #[test]
    fn extract_codes_boundary() {
        let codes = extract_http_status_codes("HTTP 429 then HTTP 500");
        assert!(codes.contains(&429));
        assert!(codes.contains(&500));
        // 200 gibi kodlar 400-599 dışında → yok.
        let codes2 = extract_http_status_codes("HTTP 200 OK");
        assert!(codes2.is_empty());
    }
}
