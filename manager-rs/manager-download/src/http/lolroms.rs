//! Gap-4 4f — lolroms.com indirme (reqwest fallback).
//!
//! Python `network/lolroms.py` karşılığı. Tam external-tool (curl/wget subprocess)
//! implementasyonu sonraki sprint out-of-scope; burada reqwest tabanlı sadelik
//! uygulanır: parent sayfa GET ile cookie/referer ısınması + dosya indirme. Mevcut
//! retry/stream/guards hattı (`download_async`) kullanılır, yeni motor yazılmaz.

use percent_encoding::{percent_decode_str, percent_encode, AsciiSet, NON_ALPHANUMERIC};

/// Python `safe="/@:$&'()*+,;=-._~"` (path) karşılığı — `/` ve yazım karakterleri
/// encode edilmez.
const LOLROMS_PATH_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'/')
    .remove(b'@')
    .remove(b':')
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
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Python `safe="=&:$,;+-._~!*'()"` (query) karşılığı.
const LOLROMS_QUERY_SAFE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'=')
    .remove(b'&')
    .remove(b':')
    .remove(b'$')
    .remove(b',')
    .remove(b';')
    .remove(b'+')
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~')
    .remove(b'!')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

/// LOLROMs URL'i mi? (host `lolroms.com` ya da `*.lolroms.com`). Python
/// `_is_lolroms_url` (host endswith) — ama `notlolroms.com` tuzağına düşmemek için
/// tam eşleşme veya `.lolroms.com` alt-etki alanı denetlenir.
pub fn is_lolroms_url(url: &str) -> bool {
    match url::Url::parse(url) {
        Ok(u) => u
            .host_str()
            .map(|h| {
                let h = h.to_ascii_lowercase();
                h == "lolroms.com" || h.ends_with(".lolroms.com")
            })
            .unwrap_or(false),
        Err(_) => false,
    }
}

/// URL'yi normalize eder: path/query önce decode edilip sonra lolroms SAFE set ile
/// yeniden encode edilir (Python `_normalize_lolroms_url`). Parse edilemezse olduğu
/// gibi döner.
pub fn normalize_lolroms_url(url: &str) -> String {
    let mut u = match url::Url::parse(url) {
        Ok(u) => u,
        Err(_) => return url.to_string(),
    };
    let raw_path = u.path().to_string();
    let norm_path = percent_encode(
        percent_decode_str(&raw_path)
            .collect::<Vec<u8>>()
            .as_slice(),
        LOLROMS_PATH_SAFE,
    )
    .to_string();
    u.set_path(&norm_path);
    if let Some(q) = u.query() {
        let norm_q = percent_encode(
            percent_decode_str(q).collect::<Vec<u8>>().as_slice(),
            LOLROMS_QUERY_SAFE,
        )
        .to_string();
        u.set_query(Some(&norm_q));
    }
    u.to_string()
}

/// İndirilecek dosyanın parent dizin URL'si (Python `_build_lolroms_parent_url`).
/// Parent fetch, cookie/referer ısınması için kullanılır.
pub fn parent_url(url: &str) -> String {
    let norm = normalize_lolroms_url(url);
    match url::Url::parse(&norm) {
        Ok(mut u) => {
            let path = u.path().to_string();
            let parent = path
                .rsplit_once('/')
                .map(|(p, _)| p)
                .unwrap_or("")
                .to_string();
            let parent = format!("{}/", parent.trim_end_matches('/'));
            u.set_path(&parent);
            u.set_query(None);
            u.set_fragment(None);
            u.to_string()
        }
        Err(_) => norm,
    }
}

/// LOLROMs isteği için browser benzeri header seti (Python external-tool komut
/// satırı header'larının eşleniği). `referer` Referer başlığı olur.
pub fn lolroms_headers(referer: &str) -> Vec<(String, String)> {
    vec![
        (
            "User-Agent".to_string(),
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36".to_string(),
        ),
        ("Accept".to_string(), "application/octet-stream,*/*".to_string()),
        (
            "Accept-Language".to_string(),
            "fr-FR,fr;q=0.9,en;q=0.8".to_string(),
        ),
        ("Referer".to_string(), referer.to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_lolroms_host() {
        assert!(is_lolroms_url("https://lolroms.com/x.zip"));
        assert!(is_lolroms_url("http://www.lolroms.com/dir/f"));
        assert!(is_lolroms_url("https://sub.lolroms.com/x"));
        assert!(!is_lolroms_url("https://notlolroms.com/x"));
        assert!(!is_lolroms_url("https://lolroms.com.evil.example/x"));
        assert!(!is_lolroms_url("https://vimm.net/x"));
    }

    #[test]
    fn normalizes_path_and_query() {
        // decode → re-encode; SAFE set dışı karakterler encode edilir.
        let in_url = "https://lolroms.com/dir/file%20name.zip?a=1+b&c=d%2Fe";
        let out = normalize_lolroms_url(in_url);
        assert!(out.contains("/dir/file%20name.zip"), "path: {out}");
        // '/' query içinde korunur (LOLROMS_QUERY_SAFE), '+' encode edilmez.
        assert!(out.contains("a=1+b"), "query: {out}");
        assert!(out.contains("c=d%2Fe"), "query slash: {out}");
        // yeniden normalize edilince idempotent olmalı.
        let again = normalize_lolroms_url(&out);
        assert_eq!(again, out);
    }

    #[test]
    fn parent_strips_file_keeps_trailing_slash() {
        assert_eq!(
            parent_url("https://lolroms.com/games/nes/rom.zip"),
            "https://lolroms.com/games/nes/"
        );
        assert_eq!(
            parent_url("https://lolroms.com/rom.zip"),
            "https://lolroms.com/"
        );
    }

    #[test]
    fn headers_shape() {
        let h = lolroms_headers("https://lolroms.com/");
        let map: std::collections::HashMap<&str, &str> =
            h.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
        assert!(map.contains_key("User-Agent"));
        assert_eq!(map.get("Referer"), Some(&"https://lolroms.com/"));
        assert_eq!(map.get("Accept"), Some(&"application/octet-stream,*/*"));
    }
}
