//! Gap-4 4c — vimm.net form/mediaId çözümü.
//!
//! Python `network/http_download.py::_extract_vimm_download_info` /
//! `_fetch_vimm_download_info` / `_get_vimm_file_size` karşılığı. Vimm sayfası GET
//! edilir, `dl_form` formundaki `action` + `mediaId` çıkarılır; indirme URL'si kurulur.
//! HEAD ile Content-Disposition'dan gerçek dosya adı + Content-Length alınır.

use std::sync::OnceLock;

use regex::Regex;

/// Vimm indirme bilgisi (Python dönüş dict'i karşılığı).
#[derive(Debug, Clone, Default)]
pub struct VimmDownloadInfo {
    pub media_id: String,
    pub base_download_url: String,
    pub download_url: String,
    /// Sayfa içinden çözülen boyut ipucu (byte).
    pub size_hint: u64,
    /// HEAD Content-Disposition'tan çözülen gerçek dosya adı.
    pub real_filename: Option<String>,
}

fn re_form_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)<form\b[^>]*\bid\s*=\s*["']dl_form["'][^>]*>"#).unwrap())
}

fn re_action() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)\baction\s*=\s*(["'])(.*?)["']"#).unwrap())
}

fn re_form_block() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(r#"(?i)<form\b[^>]*\bid\s*=\s*["']dl_form["'][^>]*>(.*?)</form>"#).unwrap()
    })
}

fn re_media_a() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r#"(?i)<input\b[^>]*\bname\s*=\s*["']mediaId["'][^>]*\bvalue\s*=\s*(["'])(.*?)["']"#,
        )
        .unwrap()
    })
}

fn re_media_b() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r#"(?i)<input\b[^>]*\bvalue\s*=\s*(["'])([0-9]+)["'][^>]*\bname\s*=\s*["']mediaId["']"#,
        )
        .unwrap()
    })
}

fn re_dl_size() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)\bid\s*=\s*["']dl_size["'][^>]*>\s*([^<]+?)\s*<"#).unwrap())
}

fn re_js_media() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)\blet\s+media\s*=\s*\[\{"ID":([0-9]+)"#).unwrap())
}

fn re_js_zipped() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#""ZippedText":"([^"]+)""#).unwrap())
}

fn re_cd_filename() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"(?i)filename\*?=(?:UTF-8'')?["']?([^"';]+)"#).unwrap())
}

/// HTML varlık referanslarını çözür (Python `html.unescape` eşleniği, sınırlı).
fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut ent = String::new();
            for n in chars.by_ref() {
                if n == ';' {
                    break;
                }
                ent.push(n);
            }
            match ent.as_str() {
                "amp" => out.push('&'),
                "lt" => out.push('<'),
                "gt" => out.push('>'),
                "quot" => out.push('"'),
                "apos" | "#39" => out.push('\''),
                other if other.starts_with('#') => {
                    let code = other[1..].trim_start_matches('x').trim_start_matches('X');
                    if let Ok(cp) = u32::from_str_radix(
                        code,
                        if other.contains('x') || other.contains('X') {
                            16
                        } else {
                            10
                        },
                    ) {
                        if let Some(ch) = char::from_u32(cp) {
                            out.push(ch);
                        } else {
                            out.push('?');
                        }
                    } else {
                        out.push('?');
                    }
                }
                _ => out.push('?'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// "1.2 GB" / "512 MB" / "12345678" gibi boyut string'ini byte'a çevirir.
fn parse_size_to_bytes(s: &str) -> u64 {
    let s = s.trim();
    let (num_part, unit) = match s.rsplit_once(char::is_whitespace) {
        Some((n, u)) => (n, Some(u)),
        None => (s, None),
    };
    let num: f64 = num_part.replace(',', "").parse().unwrap_or(0.0);
    let mult: u64 = match unit.map(|u| u.to_ascii_lowercase()) {
        Some(u) if u.starts_with("t") => 1u64 << 40,
        Some(u) if u.starts_with("g") => 1u64 << 30,
        Some(u) if u.starts_with("m") => 1u64 << 20,
        Some(u) if u.starts_with("k") => 1u64 << 10,
        _ => 1,
    };
    (num * mult as f64) as u64
}

/// HTML sayfasından vimm form bilgisini çıkarır (Python `_extract_vimm_download_info`).
/// `page_url` göreceli action'ı mutlak URL'e çevirir.
pub fn extract_vimm_download_info(html: &str, page_url: &str) -> Option<VimmDownloadInfo> {
    let form_tag = re_form_tag().find(html)?;
    let action = re_action()
        .captures(form_tag.as_str())
        .and_then(|c| c.get(2))
        .map(|m| html_unescape(m.as_str()).trim().to_string())
        .unwrap_or_default();

    let form_block = re_form_block()
        .captures(html)
        .and_then(|c| c.get(2))
        .map(|m| m.as_str())
        .unwrap_or(html);

    let mut media_id = re_media_a()
        .captures(form_block)
        .and_then(|c| c.get(2))
        .or_else(|| re_media_b().captures(form_block).and_then(|c| c.get(2)))
        .map(|m| html_unescape(m.as_str()).trim().to_string())
        .unwrap_or_default();
    if media_id.is_empty() {
        if let Some(c) = re_js_media().captures(html).and_then(|c| c.get(1)) {
            media_id = c.as_str().trim().to_string();
        }
    }

    let mut size_hint = re_dl_size()
        .captures(html)
        .and_then(|c| c.get(1))
        .map(|m| parse_size_to_bytes(m.as_str()))
        .unwrap_or(0);
    if size_hint == 0 {
        if let Some(c) = re_js_zipped().captures(html).and_then(|c| c.get(1)) {
            size_hint = parse_size_to_bytes(c.as_str());
        }
    }

    if action.is_empty() || media_id.is_empty() {
        return None;
    }

    let base = url::Url::parse(page_url).ok()?.join(&action).ok()?;
    let base_download_url = base.to_string();
    let separator = if base_download_url.contains('?') {
        '&'
    } else {
        '?'
    };
    let download_url = format!("{}{}mediaId={}", base_download_url, separator, media_id);

    Some(VimmDownloadInfo {
        media_id,
        base_download_url,
        download_url,
        size_hint,
        real_filename: None,
    })
}

/// Sayfayı GET'ler, parse eder (Python `_fetch_vimm_download_info` async eşleniği).
/// `None` → vimm değil ya da parse başarısız.
pub async fn fetch_vimm_download_info(
    client: &reqwest::Client,
    url: &str,
) -> Option<VimmDownloadInfo> {
    if !url.to_lowercase().contains("vimm.net") {
        return None;
    }
    let resp = client.get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let html = resp.text().await.ok()?;
    extract_vimm_download_info(&html, url)
}

/// HEAD ile boyut + gerçek dosya adı çözür (Python `_get_vimm_file_size`).
/// Dönüş: `(size_bytes, real_filename)`.
pub async fn fetch_vimm_file_size(
    client: &reqwest::Client,
    info: &VimmDownloadInfo,
    page_url: &str,
) -> (u64, Option<String>) {
    if info.download_url.is_empty() {
        return (0, None);
    }
    let resp = client
        .head(&info.download_url)
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.124 Safari/537.36",
        )
        .header("Referer", page_url)
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let mut real = None;
            if let Some(cd) = r.headers().get(reqwest::header::CONTENT_DISPOSITION) {
                if let Ok(s) = cd.to_str() {
                    if let Some(c) = re_cd_filename().captures(s).and_then(|c| c.get(1)) {
                        real = Some(c.as_str().trim().to_string());
                    }
                }
            }
            let len = r
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            if len > 0 {
                return (len, real);
            }
            if info.size_hint > 0 {
                return (info.size_hint, real);
            }
            (0, real)
        }
        _ => (info.size_hint, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stub_returns_none_on_empty() {
        assert!(extract_vimm_download_info("<html></html>", "https://vimm.net/x").is_none());
    }

    #[test]
    fn extracts_form_media_id_and_builds_url() {
        let html = r#"
            <html><body>
            <form id="dl_form" action="/roms/download/42" method="post">
                <input type="hidden" name="mediaId" value="12345">
                <div id="dl_size">512 MB</div>
            </form>
            </body></html>"#;
        let info = extract_vimm_download_info(html, "https://vimm.net/roms/nes/1").unwrap();
        assert_eq!(info.media_id, "12345");
        assert_eq!(info.base_download_url, "https://vimm.net/roms/download/42");
        assert_eq!(
            info.download_url,
            "https://vimm.net/roms/download/42?mediaId=12345"
        );
        assert_eq!(info.size_hint, 512 * 1024 * 1024);
    }

    #[test]
    fn extracts_value_before_name_order() {
        let html = r#"<form id="dl_form" action="/d/9"><input value="777" name="mediaId"></form>"#;
        let info = extract_vimm_download_info(html, "https://vimm.net/p").unwrap();
        assert_eq!(info.media_id, "777");
        assert_eq!(info.download_url, "https://vimm.net/d/9?mediaId=777");
    }

    #[test]
    fn js_fallback_media_id() {
        let html = r#"<form id="dl_form" action="/x"></form><script>let media = [{"ID":999,"x":1}]</script>"#;
        let info = extract_vimm_download_info(html, "https://vimm.net/p").unwrap();
        assert_eq!(info.media_id, "999");
    }

    #[test]
    fn no_form_means_none() {
        assert!(
            extract_vimm_download_info("<form id='other'></form>", "https://vimm.net/p").is_none()
        );
    }
}
