//! Gap-4 4d — archive.org cookie/metadata/alt-URL çözümü.
//!
//! Python `network/archive_org.py` + `queue.py` archive hazırlık bloğu karşılığı.
//! `archive.org/download/{id}/{arc}.zip/{inner}` URL'leri için metadata API'sinden
//! (`/metadata/{id}`) `server`+`dir` alınır; bunlardan `view_archive.php` alt-URL'i
//! üretilir ve 401/403'te fallback olarak denenir.

use percent_encoding::{percent_decode_str, percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// Python `quote(..., safe="/@:$&'()*+,;=-._~")` eşleniği: `/` ve çoğu yazım
/// karakteri encode edilmez.
const SAFE_SLASH: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b':')
    .remove(b'@')
    .remove(b'$')
    .remove(b'&')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')')
    .remove(b'*')
    .remove(b'+')
    .remove(b',')
    .remove(b';')
    .remove(b'=')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'-');

/// Archive.org metadata bilgisi (`/metadata/{id}` JSON yanıtı).
#[derive(Debug, Clone, Default)]
pub struct ArchiveMeta {
    pub server: Option<String>,
    pub directory: Option<String>,
    pub is_dark: bool,
    pub files: Vec<String>,
}

/// `archive.org/download/{id}/...` URL'ini parçalar (Python `_split_archive_org_path`).
/// Yalnızca arşiv üyesi (`.zip/.rar/.7z` + iç yol) belirlenebiliyorsa `Some` döner.
pub fn split_archive_org_path(url: &str) -> Option<(String, String, String)> {
    let parts: Vec<&str> = url.splitn(2, "/download/").collect();
    if parts.len() != 2 {
        return None;
    }
    let after = parts[1];
    let identifier = after.split('/').next()?.to_string();
    if identifier.is_empty() {
        return None;
    }
    let rest = &after[identifier.len()..];
    let rest = rest.strip_prefix('/').unwrap_or(rest);
    let rest_decoded = percent_decode_str(rest).decode_utf8_lossy().to_string();
    let (first_seg, remainder) = rest_decoded.split_once('/')?;
    let lower = first_seg.to_lowercase();
    if !lower.ends_with(".zip") && !lower.ends_with(".rar") && !lower.ends_with(".7z") {
        return None;
    }
    Some((identifier, first_seg.to_string(), remainder.to_string()))
}

/// Archive.org indirme yolunu normalize eder (Python `_normalize_archive_org_download_path`).
pub fn normalize_archive_org_download_path(identifier: &str, rest: &str) -> String {
    let rest = rest.trim_start_matches('/');
    match rest.find('/') {
        None => {
            let enc = percent_encode(rest.as_bytes(), SAFE_SLASH).to_string();
            format!("/download/{identifier}/{enc}")
        }
        Some(idx) => {
            let archive_name_raw = &rest[..idx];
            let member_raw = &rest[idx + 1..];
            let archive_name =
                percent_decode_str(archive_name_raw).decode_utf8_lossy().to_string();
            let lower = archive_name.to_lowercase();
            if lower.ends_with(".zip") || lower.ends_with(".rar") || lower.ends_with(".7z") {
                let enc_name =
                    percent_encode(archive_name.as_bytes(), SAFE_SLASH).to_string();
                let enc_member = percent_encode(
                    percent_decode_str(member_raw).decode_utf8_lossy().as_bytes(),
                    SAFE_SLASH,
                )
                .to_string()
                .to_string();
                format!("/download/{identifier}/{enc_name}/{enc_member}")
            } else {
                let enc = percent_encode(
                    percent_decode_str(rest).decode_utf8_lossy().as_bytes(),
                    SAFE_SLASH,
                )
                .to_string()
                .to_string();
                format!("/download/{identifier}/{enc}")
            }
        }
    }
}

/// URL'in `scheme://host` kısmını döndürür (metadata/alt-URL tabanı için).
fn url_origin(url: &str) -> String {
    if let Ok(u) = url::Url::parse(url) {
        if let Some(host) = u.host_str() {
            return format!("{}://{}", u.scheme(), host);
        }
    }
    "https://archive.org".to_string()
}

/// Alt-URL üretir (Python `view_archive.php` eşleniği). `scheme` isteğin şemasını
/// izler (test http, üretim https).
pub fn build_view_archive_url(
    scheme: &str,
    server: &str,
    directory: &str,
    name: &str,
    inner: &str,
) -> String {
    let archive_path = format!("{directory}/{name}");
    format!(
        "{scheme}://{server}/view_archive.php?archive={}&file={}",
        percent_encode(archive_path.as_bytes(), SAFE_SLASH).to_string(),
        percent_encode(inner.as_bytes(), SAFE_SLASH).to_string(),
    )
}

/// Metadata'dan alt-URL listesi (Python `archive_alt_urls.insert(0, view_url)` eşleniği).
pub fn build_alt_urls(url: &str, meta: &ArchiveMeta) -> Vec<String> {
    let scheme = url::Url::parse(url)
        .ok()
        .map(|u| u.scheme().to_string())
        .unwrap_or_else(|| "https".to_string());
    let mut out = Vec::new();
    if let (Some(server), Some(dir)) = (&meta.server, &meta.directory) {
        if let Some((_id, name, inner)) = split_archive_org_path(url) {
            out.push(build_view_archive_url(&scheme, server, dir, &name, &inner));
        }
    }
    out
}

/// Cookie'yi `RGSX_ARCHIVE_ORG_COOKIE_PATH` dosyasından okur (Python `load_archive_org_cookie`).
/// "Cookie: ..." ön ekini soyar.
pub fn load_archive_org_cookie() -> Option<String> {
    let path = std::env::var("RGSX_ARCHIVE_ORG_COOKIE_PATH").ok()?;
    let value = std::fs::read_to_string(&path).ok()?.trim().to_string();
    if value.is_empty() {
        return None;
    }
    let value = value
        .strip_prefix("Cookie:")
        .map(|s| s.trim().to_string())
        .unwrap_or(value);
    Some(value)
}

/// `https://archive.org/metadata/{identifier}` GET'ler, `server`/`dir`/`is_dark`/`files` çıkarır.
pub async fn fetch_archive_metadata(
    client: &reqwest::Client,
    url: &str,
    cookie: Option<&str>,
) -> Option<ArchiveMeta> {
    let identifier = url.split("/download/").nth(1)?.split('/').next()?.to_string();
    if identifier.is_empty() {
        return None;
    }
    let meta_url = format!("{}/metadata/{identifier}", url_origin(url));
    let mut req = client.get(&meta_url);
    if let Some(c) = cookie {
        if !c.is_empty() {
            req = req.header("Cookie", c);
        }
    }
    let resp = req.send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let text = resp.text().await.ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let server = v.get("server").and_then(|s| s.as_str()).map(|s| s.to_string());
    let directory = v.get("dir").and_then(|s| s.as_str()).map(|s| s.to_string());
    let is_dark = v
        .get("metadata")
        .and_then(|m| m.get("is_dark"))
        .and_then(|d| d.as_str())
        == Some("true");
    let files = v
        .get("files")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();
    Some(ArchiveMeta {
        server,
        directory,
        is_dark,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_split_none() {
        assert!(split_archive_org_path("https://archive.org/download/x/y.zip").is_none());
    }

    #[test]
    fn split_finds_archive_and_inner() {
        let r = split_archive_org_path(
            "https://archive.org/download/ident/game.zip/inner/folder/file.bin",
        )
        .unwrap();
        assert_eq!(r.0, "ident");
        assert_eq!(r.1, "game.zip");
        assert_eq!(r.2, "inner/folder/file.bin");
    }

    #[test]
    fn split_rejects_non_archive() {
        assert!(split_archive_org_path("https://archive.org/download/ident/readme.txt").is_none());
    }

    #[test]
    fn normalize_keeps_slash_safe() {
        let n = normalize_archive_org_download_path("ident", "game.zip/sub/file.bin");
        assert_eq!(n, "/download/ident/game.zip/sub/file.bin");
    }

    #[test]
    fn view_url_shape() {
        let u = build_view_archive_url("https", "ia800.us.archive.org", "/dir", "f.zip", "inner/a.bin");
        assert!(u.contains("view_archive.php?archive="));
        assert!(u.contains("&file="));
        // SAFE_SLASH: '/' encode edilmez.
        assert!(u.contains("/dir/f.zip"));
        assert!(u.contains("inner/a.bin"));
    }

    #[test]
    fn alt_urls_from_meta() {
        let meta = ArchiveMeta {
            server: Some("ia1.archive.org".into()),
            directory: Some("/d".into()),
            is_dark: false,
            files: vec![],
        };
        let urls = build_alt_urls(
            "https://archive.org/download/ident/game.zip/inner/x.bin",
            &meta,
        );
        assert_eq!(urls.len(), 1);
        assert!(urls[0].contains("view_archive.php?archive="));
        assert!(urls[0].contains("/d/game.zip"));
    }
}
