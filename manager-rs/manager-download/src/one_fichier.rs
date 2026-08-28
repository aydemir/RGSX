//! TASK-002-gap-11 — 1fichier provider zinciri.
//!
//! `network/one_fichier.py` OF0..OF18 parity:
//! - Faz1: Provider sıralı fallback 1F→AD→DL→RD→TB→FREE, ApiKeys, DedupCache, history fields
//! - Faz2: 1F direkt `file/info.cgi` → `get_token.cgi` (OF5→OF9) + FREE mode pure helpers
//!   (wait seconds, visible text, normalize, block reason, form parse, candidate extract, sanitize)

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use regex::Regex;

// ---------------------------------------------------------------------------
// Provider + ApiKeys + Dedup (Faz1)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Faz2: 1Fichier direkt — file/info.cgi + get_token.cgi (OF5→OF9)
// ---------------------------------------------------------------------------

pub const ONE_FICHIER_INFO_URL: &str = "https://api.1fichier.com/v1/file/info.cgi";
pub const ONE_FICHIER_TOKEN_URL: &str = "https://api.1fichier.com/v1/download/get_token.cgi";

/// 1Fichier file/info sonucu.
#[derive(Debug, Clone)]
pub struct OneFichierFileInfo {
    pub filename: String,
    pub size: Option<u64>,
    /// Ham API yanıtı (debug / hata mesajı).
    pub raw_json: serde_json::Value,
}

/// 1Fichier direkt zinciri hatası (OF5→OF9). Python'daki friendly msg haritası korunur.
#[derive(Debug, thiserror::Error)]
pub enum OneFichierDirectError {
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String, raw: Option<String> },
    #[error("API hatası: {0}")]
    Api(String),
    #[error("dosya bulunamadı: {0}")]
    NotFound(String),
    #[error("dosya adı alınamadı")]
    MissingFilename,
    #[error("indirme URL'si alınamadı")]
    MissingUrl,
    #[error("ağ hatası: {0}")]
    Network(String),
    #[error("geçersiz yanıt: {0}")]
    InvalidResponse(String),
}

pub fn sanitize_filename(name: &str) -> String {
    // Python `sanitize_filename` eşleniği (basit): path ayırıcı + ':' yasak, kontrol karakterleri '_' .
    let mut out = String::with_capacity(name.len());
    for c in name.chars() {
        if c == '/' || c == '\\' || c == ':' || c == '\0' {
            out.push('_');
        } else if c.is_control() {
            out.push('_');
        } else {
            out.push(c);
        }
    }
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() { "_".to_string() } else { trimmed }
}

/// 1Fichier `file/info.cgi` çağırır (OF6). Başarıda `OneFichierFileInfo`, hatada `OneFichierDirectError`.
pub async fn onefichier_file_info(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<OneFichierFileInfo, OneFichierDirectError> {
    let link = url.split("&af=").next().unwrap_or(url);
    let payload = serde_json::json!({ "url": link, "pretty": 1 });
    let resp = client
        .post(ONE_FICHIER_INFO_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| OneFichierDirectError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| OneFichierDirectError::Network(e.to_string()))?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if status != 200 {
        let raw_err = json.get("message").or_else(|| json.get("error")).or_else(|| json.get("status"))
            .and_then(|v| v.as_str()).unwrap_or("");
        // Friendly harita (Python one_fichier.py ile parity)
        let friendly = if raw_err == "Bad token" {
            "1F: Clé API 1fichier invalide".to_string()
        } else if !raw_err.is_empty() {
            format!("1F: {}", raw_err)
        } else if status == 403 {
            "1F: Accès refusé (403)".to_string()
        } else if status == 401 {
            "1F: Non autorisé (401)".to_string()
        } else {
            format!("1F: Erreur HTTP {}", status)
        };
        // Resource not found özel durumu
        if json.get("error").and_then(|v| v.as_str()) == Some("Resource not found") {
            return Err(OneFichierDirectError::NotFound(friendly));
        }
        return Err(OneFichierDirectError::Http { status, message: friendly, raw: Some(text) });
    }
    if json.get("error").and_then(|v| v.as_str()) == Some("Resource not found") {
        return Err(OneFichierDirectError::NotFound("1F: File not found".into()));
    }
    let filename = json.get("filename").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if filename.is_empty() {
        return Err(OneFichierDirectError::MissingFilename);
    }
    let size = json.get("size").and_then(|v| {
        if let Some(n) = v.as_u64() { Some(n) }
        else if let Some(s) = v.as_str() { s.parse::<u64>().ok() }
        else { None }
    });
    Ok(OneFichierFileInfo { filename, size, raw_json: json })
}

/// 1Fichier `get_token.cgi` çağırır (OF9). Başarıda final_url döner.
pub async fn onefichier_get_token(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<String, OneFichierDirectError> {
    let link = url.split("&af=").next().unwrap_or(url);
    let payload = serde_json::json!({ "url": link, "pretty": 1 });
    let resp = client
        .post(ONE_FICHIER_TOKEN_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| OneFichierDirectError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| OneFichierDirectError::Network(e.to_string()))?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if status != 200 {
        let raw_err = json.get("message").or_else(|| json.get("status"))
            .and_then(|v| v.as_str()).unwrap_or("");
        let mapped = match raw_err {
            "Bad token" => Some("1F: Clé API invalide"),
            "Must be a customer (Premium, Access) #236" => Some("1F: Compte Premium requis"),
            _ => None,
        };
        let friendly = if let Some(m) = mapped {
            m.to_string()
        } else if status == 403 {
            "1F: Accès refusé (403)".to_string()
        } else if status == 401 {
            "1F: Non autorisé (401)".to_string()
        } else if status >= 500 {
            format!("1F: Erreur serveur ({})", status)
        } else if !raw_err.is_empty() {
            format!("1F: {}", raw_err)
        } else {
            format!("1F: Erreur ({})", status)
        };
        return Err(OneFichierDirectError::Http { status, message: friendly, raw: Some(text) });
    }
    let j = if json.is_object() { json } else { serde_json::Value::Null };
    let final_url = j.get("url").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if final_url.is_empty() {
        return Err(OneFichierDirectError::MissingUrl);
    }
    Ok(final_url)
}

/// 1Fichier direkt zinciri: info → token. Başarıda (filename, size, final_url).
pub async fn onefichier_direct_url(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<(String, Option<u64>, String), OneFichierDirectError> {
    let info = onefichier_file_info(client, api_key, url).await?;
    let token = onefichier_get_token(client, api_key, url).await?;
    Ok((info.filename, info.size, token))
}

// ---------------------------------------------------------------------------
// Faz2: FREE mode pure helpers (OF_OFF)
// ---------------------------------------------------------------------------

fn re_wait_ct_mul() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)var\s+ct\s*=\s*(\d+)\s*\*\s*60").unwrap())
}
fn re_wait_min_fr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)(?:veuillez\s+)?patiente[rz]\s*(\d+)\s*(?:min|minute)s?\b").unwrap())
}
fn re_wait_min_en() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)please\s+wait\s*(\d+)\s*(?:min|minute)s?\b").unwrap())
}
fn re_wait_sec_fr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)(?:veuillez\s+)?patiente[rz]\s*(\d+)\s*(?:sec|secondes?|s)\b").unwrap())
}
fn re_wait_sec_en() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)please\s+wait\s*(\d+)\s*(?:sec|seconds?)\b").unwrap())
}
fn re_wait_ct_plain() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?i)var\s+ct\s*=\s*(\d+)\s*;").unwrap())
}

/// Python `extract_wait_seconds_1f` parity: HTML → bekleme saniyesi (0 = yok).
pub fn extract_wait_seconds(html: &str) -> u64 {
    if let Some(c) = re_wait_ct_mul().captures(html).and_then(|c| c.get(1)) {
        if let Ok(v) = c.as_str().parse::<u64>() { return v * 60; }
    }
    if let Some(c) = re_wait_min_fr().captures(html).and_then(|c| c.get(1)) {
        if let Ok(v) = c.as_str().parse::<u64>() { return v * 60; }
    }
    if let Some(c) = re_wait_min_en().captures(html).and_then(|c| c.get(1)) {
        if let Ok(v) = c.as_str().parse::<u64>() { return v * 60; }
    }
    if let Some(c) = re_wait_sec_fr().captures(html).and_then(|c| c.get(1)) {
        if let Ok(v) = c.as_str().parse::<u64>() { return v; }
    }
    if let Some(c) = re_wait_sec_en().captures(html).and_then(|c| c.get(1)) {
        if let Ok(v) = c.as_str().parse::<u64>() { return v; }
    }
    if let Some(c) = re_wait_ct_plain().captures(html).and_then(|c| c.get(1)) {
        if let Ok(v) = c.as_str().parse::<u64>() { return v; }
    }
    0
}

fn re_script() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap())
}
fn re_style() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap())
}
fn re_tag() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"(?is)<[^>]+>").unwrap())
}
fn re_ws() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\s+").unwrap())
}

/// HTML visible text (Python `_extract_visible_text_from_html`).
pub fn extract_visible_text(html: &str) -> String {
    if html.is_empty() { return String::new(); }
    let mut s = re_script().replace_all(html, " ").to_string();
    s = re_style().replace_all(&s, " ").to_string();
    s = re_tag().replace_all(&s, " ").to_string();
    s = html_unescape(&s);
    s = s.replace('\u{00a0}', " ");
    re_ws().replace_all(s.trim(), " ").to_string()
}

fn html_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&' {
            let mut ent = String::new();
            let mut found_semi = false;
            for ch in chars.by_ref() {
                if ch == ';' { found_semi = true; break; }
                ent.push(ch);
                if ent.len() > 16 { break; }
            }
            if found_semi {
                match ent.as_str() {
                    "amp" => out.push('&'),
                    "lt" => out.push('<'),
                    "gt" => out.push('>'),
                    "quot" => out.push('"'),
                    "apos" | "#39" => out.push('\''),
                    "nbsp" => out.push(' '),
                    other if other.starts_with('#') => {
                        let code = other[1..].trim_start_matches('x').trim_start_matches('X');
                        let radix = if other.contains('x') || other.contains('X') { 16 } else { 10 };
                        if let Ok(cp) = u32::from_str_radix(code, radix) {
                            if let Some(ch) = char::from_u32(cp) { out.push(ch); } else { out.push('?'); }
                        } else { out.push('?'); }
                    }
                    _ => { out.push('&'); out.push_str(&ent); out.push(';'); }
                }
            } else {
                out.push('&');
                out.push_str(&ent);
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Python `_normalize_1fichier_text` parity: NFKD → ascii ignore → lower → ws normalize.
pub fn normalize_1fichier_text(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let nfkd: String = text.nfkd().collect();
    let ascii: String = nfkd.chars().filter(|c| c.is_ascii()).collect();
    let lower = ascii.to_ascii_lowercase();
    re_ws().replace_all(lower.trim(), " ").to_string()
}

const FREE_UPGRADE_ADVICE: &str = "For unlimited, on-demand, full-speed downloads, you need a premium account or debrid service and must enter its API key in RGSX.";

fn append_upgrade_advice(msg: &str) -> String {
    let base = msg.trim();
    if base.is_empty() { return FREE_UPGRADE_ADVICE.to_string(); }
    format!("{}\n{}", base, FREE_UPGRADE_ADVICE)
}

/// Python `_extract_1fichier_free_mode_block_reason` parity.
pub fn extract_free_block_reason(html: &str) -> Option<String> {
    let visible = extract_visible_text(html);
    let norm = normalize_1fichier_text(&visible);
    if norm.is_empty() { return None; }
    let is_guest_slots = norm.contains("telechargement gratuit est temporairement limite")
        && norm.contains("identifiez-vous immediatement");
    let is_guest_slots_en = norm.contains("free download is temporarily limited")
        && norm.contains("all free slots for guests are currently used");
    if is_guest_slots || is_guest_slots_en {
        return Some(append_upgrade_advice(
            "1fichier: free guest download is temporarily unavailable (all slots are currently in use). Please try again later."
        ));
    }
    if norm.contains("identifiez-vous immediatement pour continuer votre telechargement")
        || norm.contains("sign in immediately to continue your download") {
        return Some(append_upgrade_advice(
            "1fichier: this download is not available in the application right now. Please try again later."
        ));
    }
    None
}

/// FREE form `id="f1"` içinden input alanlarını çıkarır (checkbox/radio yalnız checked).
pub fn parse_free_form_data(html: &str) -> Option<HashMap<String, String>> {
    let re_form = Regex::new(r#"(?is)<form[^>]*id\s*=\s*["']f1["'][^>]*>(.*?)</form>"#).ok()?;
    let caps = re_form.captures(html)?;
    let inner = caps.get(1)?.as_str();
    let re_input = Regex::new(r"(?i)<input[^>]+>").ok()?;
    let re_name = Regex::new(r#"name\s*=\s*["']([^"']+)"#).ok()?;
    let re_value = Regex::new(r#"value\s*=\s*["']([^"']*)"#).ok()?;
    let re_type = Regex::new(r#"type\s*=\s*["']([^"']+)"#).ok()?;
    let mut data = HashMap::new();
    for m in re_input.find_iter(inner) {
        let inp = m.as_str();
        let name = match re_name.captures(inp).and_then(|c| c.get(1)) { Some(v) => v.as_str().to_string(), None => continue };
        let typ = re_type.captures(inp).and_then(|c| c.get(1)).map(|v| v.as_str().to_ascii_lowercase()).unwrap_or_else(|| "text".to_string());
        if (typ == "checkbox" || typ == "radio") && !inp.to_ascii_lowercase().contains("checked") {
            continue;
        }
        let value = re_value.captures(inp).and_then(|c| c.get(1)).map(|v| html_unescape(v.as_str())).unwrap_or_default();
        data.insert(name, value);
    }
    if data.is_empty() { None } else { Some(data) }
}

/// FREE direk link adaylarını çıkarır (Python candidate_entries mantığı, ağ doğrulaması hariç).
pub fn extract_free_candidates(html: &str, page_url: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let base = url::Url::parse(page_url).ok();
    let re_anchor = Regex::new(r#"(?is)<a[^>]+href\s*=\s*["']([^"']+)["'][^>]*>(.*?)</a>"#).unwrap();
    let re_tag_strip = Regex::new(r"(?is)<[^>]+>").unwrap();
    for caps in re_anchor.captures_iter(html) {
        let href = html_unescape(caps.get(1).map(|m| m.as_str()).unwrap_or("").trim());
        let inner = caps.get(2).map(|m| m.as_str()).unwrap_or("");
        let anchor_text = re_tag_strip.replace_all(inner, " ").to_string();
        let norm = normalize_1fichier_text(&anchor_text);
        if href.is_empty() || norm.is_empty() { continue; }
        if !["download","telecharg","tlcharg","click","cliquer"].iter().any(|t| norm.contains(t)) { continue; }
        let cand = if href.starts_with("http://") || href.starts_with("https://") {
            href.clone()
        } else if let Some(b) = &base { b.join(&href).map(|u| u.to_string()).unwrap_or(href.clone()) } else { href.clone() };
        if seen.insert(cand.clone()) { out.push(cand); }
    }
    let patterns: &[&str] = &[
        r#"(?i)href\s*=\s*["']([^"']+)["'][^>]*>(?:cliquer|click|télécharger|download)"#,
        r#"(?i)href\s*=\s*["']([^"']*/dl/[^"']+)"#,
        r#"(?i)(https?://[a-z0-9.\-]*1fichier\.com/[A-Za-z0-9]{8,})"#,
    ];
    for pat in patterns {
        let re = Regex::new(pat).unwrap();
        for caps in re.captures_iter(html) {
            let raw = caps.get(1).map(|m| m.as_str()).unwrap_or("").trim();
            if raw.is_empty() { continue; }
            let cap = html_unescape(raw);
            let cand = if cap.starts_with("http://") || cap.starts_with("https://") {
                cap.clone()
            } else if let Some(b) = &base { b.join(&cap).map(|u| u.to_string()).unwrap_or(cap.clone()) } else { cap.clone() };
            if seen.insert(cand.clone()) { out.push(cand); }
        }
    }
    out.into_iter().filter(|c| {
        let low = c.to_ascii_lowercase();
        !["/register","/login","/inscription","/compte","/subscribe"].iter().any(|x| low.contains(x))
    }).collect()
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

    // --- Faz2 pure helpers ---

    #[test]
    fn wait_seconds_ct_mul() {
        assert_eq!(extract_wait_seconds("var ct = 2 * 60;"), 120);
        assert_eq!(extract_wait_seconds("var ct = 3*60;"), 180);
    }
    #[test]
    fn wait_seconds_minutes() {
        assert_eq!(extract_wait_seconds("Veuillez patienter 5 minutes"), 300);
        assert_eq!(extract_wait_seconds("please wait 2 minutes"), 120);
    }
    #[test]
    fn wait_seconds_seconds_and_plain() {
        assert_eq!(extract_wait_seconds("patientez 45 secondes"), 45);
        assert_eq!(extract_wait_seconds("please wait 10 seconds"), 10);
        assert_eq!(extract_wait_seconds("var ct = 90;"), 90);
        assert_eq!(extract_wait_seconds("<html>no wait</html>"), 0);
    }

    #[test]
    fn visible_text_strips_tags_and_unescapes() {
        let html = "<script>alert(1)</script><style>.x{}</style><p>Hello&nbsp;World &amp; test</p>";
        let t = extract_visible_text(html);
        assert_eq!(t, "Hello World & test");
    }

    #[test]
    fn normalize_strips_accents_and_lowers() {
        let n = normalize_1fichier_text("  Téléchargement  GRATUIT  ");
        assert_eq!(n, "telechargement gratuit");
    }

    #[test]
    fn block_reason_guest_slots_fr() {
        let html = "<div>Le téléchargement gratuit est temporairement limité. Veuillez vous identifiez-vous immediatement</div>";
        // visible contains required substrings after NFKD ascii ignoring
        let reason = extract_free_block_reason(html);
        assert!(reason.is_some());
        let msg = reason.unwrap();
        assert!(msg.contains("free guest download is temporarily unavailable"));
        assert!(msg.contains(FREE_UPGRADE_ADVICE));
    }

    #[test]
    fn block_reason_sign_in() {
        let html = "<div>Sign in immediately to continue your download</div>";
        let reason = extract_free_block_reason(html).unwrap();
        assert!(reason.contains("not available in the application"));
    }

    #[test]
    fn block_reason_none() {
        assert!(extract_free_block_reason("<p>normal page</p>").is_none());
    }

    #[test]
    fn form_data_parses_checked_only() {
        let html = r#"<form id="f1"><input name="a" value="1"><input type="checkbox" name="c" value="yes"><input type="checkbox" name="d" value="ok" checked><input name="b" value="2"></form>"#;
        let data = parse_free_form_data(html).unwrap();
        assert_eq!(data.get("a").map(|s| s.as_str()), Some("1"));
        assert_eq!(data.get("b").map(|s| s.as_str()), Some("2"));
        assert!(!data.contains_key("c"));
        assert_eq!(data.get("d").map(|s| s.as_str()), Some("ok"));
    }

    #[test]
    fn candidates_extract_anchor() {
        let html = r#"<a href="https://1fichier.com/dl/ABCDEFGH">Cliquez ici pour télécharger</a><a href="/register">register</a>"#;
        let cands = extract_free_candidates(html, "https://1fichier.com/?abc");
        assert!(cands.iter().any(|c| c.contains("/dl/")));
        assert!(!cands.iter().any(|c| c.contains("/register")));
    }

    #[test]
    fn sanitize_filename_basic() {
        assert_eq!(sanitize_filename("a/b\\c:d.txt"), "a_b_c_d.txt");
        assert_eq!(sanitize_filename("  "), "_");
        assert_eq!(sanitize_filename("game.zip"), "game.zip");
    }
}
