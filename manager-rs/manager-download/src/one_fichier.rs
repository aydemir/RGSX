//! TASK-002-gap-11 — 1fichier provider zinciri.
//!
//! `network/one_fichier.py` OF0..OF18 parity:
//! - Faz1: Provider sıralı fallback 1F→AD→DL→RD→TB→FREE, ApiKeys, DedupCache, history fields
//! - Faz2: 1F direkt `file/info.cgi` → `get_token.cgi` (OF5→OF9) + FREE mode pure helpers
//!   (wait seconds, visible text, normalize, block reason, form parse, candidate extract, sanitize)
//! - Faz3: Debrid fallback OFA→OFT (AD/DL/RD/TB) + chain orchestrator, hata haritaları, md5, refresh

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

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.prefix())
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

// ---------------------------------------------------------------------------
// Faz3: Debrid fallback zinciri OFA→OFT (AD → DL → RD → TB) + chain orchestrator
// Python one_fichier.py download_from_1fichier OFA..OFT parity.
// ---------------------------------------------------------------------------

pub const ALLDEBRID_UNLOCK_URL: &str = "https://api.alldebrid.com/v4/link/unlock";
pub const DEBRIDLINK_ADD_URL: &str = "https://debrid-link.com/api/v2/downloader/add";
pub const REALDEBRID_UNRESTRICT_URL: &str = "https://api.real-debrid.com/rest/1.0/unrestrict/link";
pub const TORBOX_CHECKCACHED_URL: &str = "https://api.torbox.app/v1/api/webdl/checkcached";
pub const TORBOX_CREATE_URL: &str = "https://api.torbox.app/v1/api/webdl/createwebdownload";
pub const TORBOX_MYLIST_URL: &str = "https://api.torbox.app/v1/api/webdl/mylist";
pub const TORBOX_REQUESTDL_URL: &str = "https://api.torbox.app/v1/api/webdl/requestdl";

/// Debrid başarı sonucu (filename fallback game_name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebridSuccess {
    pub provider: Provider,
    pub filename: String,
    pub final_url: String,
}

/// Debrid zinciri hatası (tek provider).
#[derive(Debug, thiserror::Error)]
pub enum DebridError {
    #[error("{provider:?} HTTP {status}: {message}")]
    Http { provider: Provider, status: u16, message: String, raw: Option<String> },
    #[error("{provider:?} API: {message} (code={code})")]
    Api { provider: Provider, code: String, message: String },
    #[error("{provider:?} ağ hatası: {detail}")]
    Network { provider: Provider, detail: String },
    #[error("{provider:?} final_url yok")]
    MissingUrl { provider: Provider },
    #[error("{provider:?} bulunamadı: {message}")]
    NotFound { provider: Provider, message: String },
}

// ---------- hata haritaları (pure, test edilebilir) ----------

pub fn debridlink_friendly(code: &str) -> String {
    match code {
        "badToken" => "DL: Invalid API key".into(),
        "notDebrid" => "DL: Host unavailable".into(),
        "hostNotValid" => "DL: Unsupported host".into(),
        "fileNotFound" => "DL: File not found".into(),
        "fileNotAvailable" => "DL: File temporarily unavailable".into(),
        "badFileUrl" => "DL: Invalid link".into(),
        "badFilePassword" => "DL: Invalid file password".into(),
        "notFreeHost" => "DL: Premium account only".into(),
        "maintenanceHost" => "DL: Host in maintenance".into(),
        "noServerHost" => "DL: No server available".into(),
        "maxLink" => "DL: Daily link limit reached".into(),
        "maxLinkHost" => "DL: Daily host limit reached".into(),
        "maxData" => "DL: Daily data limit reached".into(),
        "maxDataHost" => "DL: Daily host data limit reached".into(),
        "disabledServerHost" => "DL: Server or VPN not allowed".into(),
        "floodDetected" => "DL: Rate limit reached".into(),
        other => format!("DL: {other}"),
    }
}

pub fn realdebrid_friendly(code: i32, api_text: Option<&str>) -> String {
    let base = match code {
        1 => "Bad request",
        2 => "Unsupported hoster",
        3 => "Temporarily unavailable",
        4 => "File not found",
        5 => "Too many requests",
        6 => "Access denied",
        8 => "Not premium account",
        9 => "No traffic left",
        11 => "Internal error",
        20 => "Premium account only",
        _ => return api_text.map(|t| format!("RD: {t}")).unwrap_or_else(|| format!("RD: error {code}")),
    };
    // hoster_not_free → Premium account only normalizasyonu (python parity)
    if let Some(t) = api_text {
        if t.trim().eq_ignore_ascii_case("hoster_not_free") {
            return "RD: Premium account only".into();
        }
        // api_text already mapped? we return base
        let _ = t;
    }
    format!("RD: {base}")
}

pub fn torbox_friendly(code: &str, detail: Option<&str>) -> String {
    match code {
        "BAD_TOKEN" => "TB: Invalid API key".into(),
        "AUTH_ERROR" => "TB: Authentication error".into(),
        "NO_AUTH" => "TB: No credentials provided".into(),
        "PLAN_RESTRICTED_FEATURE" => "TB: Plan upgrade required".into(),
        "DOWNLOAD_TOO_LARGE" => "TB: Download too large for plan".into(),
        "MONTHLY_LIMIT" => "TB: Monthly limit reached".into(),
        "COOLDOWN_LIMIT" => "TB: Download cooldown active".into(),
        "ACTIVE_LIMIT" => "TB: Max active downloads reached".into(),
        "LINK_OFFLINE" => "TB: Link offline or inaccessible".into(),
        "ITEM_NOT_FOUND" => "TB: Item not found".into(),
        "NO_SERVERS_AVAILABLE_ERROR" => "TB: No servers available".into(),
        "DOWNLOAD_SERVER_ERROR" => "TB: Download server error".into(),
        other => {
            let d = detail.unwrap_or(other);
            if d.is_empty() { format!("TB: {other}") } else { format!("TB: {d}") }
        }
    }
}

pub fn md5_hex(s: &str) -> String {
    let digest = md5::compute(s.as_bytes());
    format!("{digest:x}")
}

// ---------- AllDebrid (OFA) ----------

pub async fn alldebrid_unlock(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<DebridSuccess, DebridError> {
    let link = url.split("&af=").next().unwrap_or(url);
    let resp = client
        .get(ALLDEBRID_UNLOCK_URL)
        .query(&[("agent", "RGSX"), ("apikey", api_key), ("link", link)])
        .send()
        .await
        .map_err(|e| DebridError::Network { provider: Provider::AllDebrid, detail: e.to_string() })?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| DebridError::Network { provider: Provider::AllDebrid, detail: e.to_string() })?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if status != 200 {
        return Err(DebridError::Http { provider: Provider::AllDebrid, status, message: format!("AD: HTTP {status}"), raw: Some(text) });
    }
    if json.get("status").and_then(|v| v.as_str()) != Some("success") {
        let raw = text.clone();
        return Err(DebridError::Api { provider: Provider::AllDebrid, code: raw.clone(), message: format!("AD: {}", json.get("error").or_else(|| json.get("message")).and_then(|v| v.as_str()).unwrap_or(&raw)) });
    }
    let data = json.get("data").cloned().unwrap_or(serde_json::Value::Null);
    let final_url = data.get("link").or_else(|| data.get("download")).or_else(|| data.get("streamingLink"))
        .and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    if final_url.is_empty() {
        return Err(DebridError::MissingUrl { provider: Provider::AllDebrid });
    }
    let filename = data.get("filename").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
    Ok(DebridSuccess { provider: Provider::AllDebrid, filename, final_url })
}

/// AllDebrid 503 sonrası link yenileme (OF11a `_refresh_alldebrid_final_url` parity) — unlock'u tekrar çağırır.
pub async fn refresh_alldebrid_url(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Option<(String, String)> {
    match alldebrid_unlock(client, api_key, url).await {
        Ok(s) => Some((s.final_url, s.filename)),
        Err(_) => None,
    }
}

// ---------- Debrid-Link (OFD) ----------

pub async fn debridlink_add(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<DebridSuccess, DebridError> {
    let link = url.split("&af=").next().unwrap_or(url);
    let payload = serde_json::json!({ "url": link });
    let resp = client
        .post(DEBRIDLINK_ADD_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| DebridError::Network { provider: Provider::DebridLink, detail: e.to_string() })?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| DebridError::Network { provider: Provider::DebridLink, detail: e.to_string() })?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if json.get("success").and_then(|v| v.as_bool()) == Some(true) {
        let value = json.get("value").cloned().unwrap_or(serde_json::Value::Null);
        let final_url = value.get("downloadUrl").or_else(|| value.get("downloadURL")).or_else(|| value.get("link")).or_else(|| value.get("url"))
            .and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        if final_url.is_empty() {
            return Err(DebridError::MissingUrl { provider: Provider::DebridLink });
        }
        let filename = value.get("name").or_else(|| value.get("filename")).and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
        return Ok(DebridSuccess { provider: Provider::DebridLink, filename, final_url });
    }
    // hata yolu
    let code = json.get("error").and_then(|v| v.as_str()).unwrap_or("");
    if !code.is_empty() {
        return Err(DebridError::Api { provider: Provider::DebridLink, code: code.to_string(), message: debridlink_friendly(code) });
    }
    if status == 401 {
        return Err(DebridError::Http { provider: Provider::DebridLink, status, message: "DL: Unauthorized (401)".into(), raw: Some(text) });
    }
    if status == 429 {
        return Err(DebridError::Http { provider: Provider::DebridLink, status, message: "DL: Rate limited (429)".into(), raw: Some(text) });
    }
    if status >= 500 {
        return Err(DebridError::Http { provider: Provider::DebridLink, status, message: format!("DL: Server error ({status})"), raw: Some(text) });
    }
    Err(DebridError::Http { provider: Provider::DebridLink, status, message: format!("DL: Unexpected status ({status})"), raw: Some(text) })
}

// ---------- RealDebrid (OFR) ----------

pub async fn realdebrid_unrestrict(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<DebridSuccess, DebridError> {
    let link = url.split("&af=").next().unwrap_or(url);
    let resp = client
        .post(REALDEBRID_UNRESTRICT_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .form(&[("link", link)])
        .send()
        .await
        .map_err(|e| DebridError::Network { provider: Provider::RealDebrid, detail: e.to_string() })?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| DebridError::Network { provider: Provider::RealDebrid, detail: e.to_string() })?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
    if status == 200 {
        if let Some(dl) = json.get("download").and_then(|v| v.as_str()) {
            let final_url = dl.trim().to_string();
            if !final_url.is_empty() {
                let filename = json.get("filename").and_then(|v| v.as_str()).unwrap_or("").trim().to_string();
                return Ok(DebridSuccess { provider: Provider::RealDebrid, filename, final_url });
            }
        }
    }
    // hata parse
    let mut code_i: Option<i32> = None;
    let mut api_text: Option<String> = None;
    if let Some(v) = json.get("error_code") {
        if let Some(n) = v.as_i64() { code_i = Some(n as i32); }
        else if let Some(s) = v.as_str() { if let Ok(n) = s.parse::<i32>() { code_i = Some(n); } }
    }
    if let Some(s) = json.get("error").and_then(|v| v.as_str()) {
        // python: error int ise code, string ise message
        if code_i.is_none() {
            if let Ok(n) = s.parse::<i32>() { code_i = Some(n); } else { api_text = Some(s.to_string()); }
        } else {
            api_text = Some(s.to_string());
        }
        if json.get("error_code").is_none() && s.eq_ignore_ascii_case("hoster_not_free") {
            api_text = Some(s.to_string());
            if code_i.is_none() { code_i = Some(20); }
        }
    }
    if let Some(c) = code_i {
        let msg = realdebrid_friendly(c, api_text.as_deref());
        return Err(DebridError::Api { provider: Provider::RealDebrid, code: c.to_string(), message: msg });
    }
    if status == 503 {
        return Err(DebridError::Http { provider: Provider::RealDebrid, status, message: "RD: service unavailable (503)".into(), raw: Some(text) });
    }
    if status >= 500 {
        return Err(DebridError::Http { provider: Provider::RealDebrid, status, message: format!("RD: server error ({status})"), raw: Some(text) });
    }
    if status == 429 {
        return Err(DebridError::Http { provider: Provider::RealDebrid, status, message: "RD: rate limited (429)".into(), raw: Some(text) });
    }
    Err(DebridError::Http { provider: Provider::RealDebrid, status, message: format!("RD: unexpected status ({status})"), raw: Some(text) })
}

// ---------- TorBox (OFT) ----------

/// TorBox webdl zinciri: checkcached (best-effort) → createwebdownload → poll mylist ≤120s → requestdl.
pub async fn torbox_webdl(
    client: &reqwest::Client,
    api_key: &str,
    url: &str,
) -> Result<DebridSuccess, DebridError> {
    let link = url.split("&af=").next().unwrap_or(url);
    let link_hash = md5_hex(link);
    // Step0 best-effort checkcached (başarısızlık non-fatal)
    let _ = client
        .get(TORBOX_CHECKCACHED_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .query(&[("hash", link_hash.as_str()), ("format", "list")])
        .send()
        .await;

    // Step1 create
    let resp = client
        .post(TORBOX_CREATE_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .form(&[("link", link)])
        .send()
        .await
        .map_err(|e| DebridError::Network { provider: Provider::TorBox, detail: e.to_string() })?;
    let status = resp.status().as_u16();
    let text = resp.text().await.map_err(|e| DebridError::Network { provider: Provider::TorBox, detail: e.to_string() })?;
    let json: serde_json::Value = serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);

    let mut webdl_id: Option<i64> = None;
    let mut err_code: Option<String> = None;
    let mut err_detail: Option<String> = None;

    if json.get("success").and_then(|v| v.as_bool()) == Some(true) {
        if let Some(data) = json.get("data") {
            webdl_id = data.get("webdl_id").or_else(|| data.get("id")).and_then(|v| v.as_i64());
        }
    } else if json.get("error").and_then(|v| v.as_str()) == Some("DUPLICATE_ITEM") {
        if let Some(data) = json.get("data") {
            webdl_id = data.get("webdl_id").or_else(|| data.get("id")).and_then(|v| v.as_i64());
        }
        if webdl_id.is_none() {
            // mylist'te hash'e göre ara
            if let Ok(list_resp) = client.get(TORBOX_MYLIST_URL).header("Authorization", format!("Bearer {api_key}")).send().await {
                if let Ok(t) = list_resp.text().await {
                    if let Ok(j) = serde_json::from_str::<serde_json::Value>(&t) {
                        if j.get("success").and_then(|v| v.as_bool()) == Some(true) {
                            if let Some(arr) = j.get("data").and_then(|v| v.as_array()) {
                                for item in arr {
                                    let h = item.get("hash").and_then(|v| v.as_str()).unwrap_or("");
                                    let orig = item.get("original_url").and_then(|v| v.as_str()).unwrap_or("");
                                    if h == link_hash || orig == link {
                                        webdl_id = item.get("id").and_then(|v| v.as_i64());
                                        if webdl_id.is_some() { break; }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if webdl_id.is_none() {
            err_code = Some("DUPLICATE_ITEM".into());
        }
    } else {
        err_code = json.get("error").and_then(|v| v.as_str()).map(|s| s.to_string());
        err_detail = json.get("detail").and_then(|v| v.as_str()).map(|s| s.to_string());
        if err_code.is_none() && status != 200 {
            err_code = Some(format!("HTTP_{status}"));
        }
    }

    if let Some(c) = err_code.clone() {
        let msg = torbox_friendly(&c, err_detail.as_deref());
        return Err(DebridError::Api { provider: Provider::TorBox, code: c, message: msg });
    }
    let wid = match webdl_id {
        Some(id) => id,
        None => {
            if status == 403 {
                return Err(DebridError::Http { provider: Provider::TorBox, status, message: "TB: Authentication failed (403)".into(), raw: Some(text) });
            }
            if status == 429 {
                return Err(DebridError::Http { provider: Provider::TorBox, status, message: "TB: Rate limited (429)".into(), raw: Some(text) });
            }
            if status >= 500 {
                return Err(DebridError::Http { provider: Provider::TorBox, status, message: format!("TB: Server error ({status})"), raw: Some(text) });
            }
            return Err(DebridError::Api { provider: Provider::TorBox, code: "NO_WEBDL_ID".into(), message: "TB: No webdl_id returned".into() });
        }
    };

    // Step2 poll mylist ≤120s, 3s interval
    let poll_deadline = std::time::Instant::now() + Duration::from_secs(120);
    let mut filename_hint = String::new();
    let mut ready = false;
    while std::time::Instant::now() < poll_deadline {
        let pr = client
            .get(TORBOX_MYLIST_URL)
            .header("Authorization", format!("Bearer {api_key}"))
            .query(&[("id", wid.to_string())])
            .send()
            .await
            .map_err(|e| DebridError::Network { provider: Provider::TorBox, detail: e.to_string() })?;
        let t = pr.text().await.map_err(|e| DebridError::Network { provider: Provider::TorBox, detail: e.to_string() })?;
        let j: serde_json::Value = serde_json::from_str(&t).unwrap_or(serde_json::Value::Null);
        if j.get("success").and_then(|v| v.as_bool()) == Some(true) {
            if let Some(data) = j.get("data") {
                let item = if let Some(arr) = data.as_array() { arr.first().cloned().unwrap_or(serde_json::Value::Null) } else { data.clone() };
                let state = item.get("download_state").and_then(|v| v.as_str()).unwrap_or("");
                let finished = item.get("download_finished").and_then(|v| v.as_bool()).unwrap_or(false);
                if ["cached","completed","uploading","done"].contains(&state) || finished {
                    filename_hint = item.get("name").or_else(|| item.get("original_name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ready = true;
                    break;
                }
                if ["error","failed","stalled"].contains(&state) {
                    return Err(DebridError::Api { provider: Provider::TorBox, code: state.to_string(), message: format!("TB: Download failed ({state})") });
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
    if !ready {
        return Err(DebridError::Api { provider: Provider::TorBox, code: "TIMEOUT".into(), message: "TB: Download not ready (timeout)".into() });
    }

    // Step3 requestdl
    let dr = client
        .get(TORBOX_REQUESTDL_URL)
        .header("Authorization", format!("Bearer {api_key}"))
        .query(&[("token", api_key), ("web_id", &wid.to_string()), ("file_id", "0")])
        .send()
        .await
        .map_err(|e| DebridError::Network { provider: Provider::TorBox, detail: e.to_string() })?;
    let t = dr.text().await.map_err(|e| DebridError::Network { provider: Provider::TorBox, detail: e.to_string() })?;
    let j: serde_json::Value = serde_json::from_str(&t).unwrap_or(serde_json::Value::Null);
    if j.get("success").and_then(|v| v.as_bool()) == Some(true) {
        if let Some(data) = j.get("data").and_then(|v| v.as_str()) {
            if !data.trim().is_empty() {
                return Ok(DebridSuccess { provider: Provider::TorBox, filename: filename_hint, final_url: data.trim().to_string() });
            }
        }
    }
    let c = j.get("error").and_then(|v| v.as_str()).unwrap_or("UNKNOWN");
    let d = j.get("detail").and_then(|v| v.as_str());
    Err(DebridError::Api { provider: Provider::TorBox, code: c.to_string(), message: torbox_friendly(c, d) })
}

// ---------- Zincir orchestrator (OF0→OFF) ----------

#[derive(Debug)]
pub enum ChainOutcome {
    Debrid(DebridSuccess),
    Free, // tüm debrid'ler başarısız → FREE scrape'e düş
}

#[derive(Debug, thiserror::Error)]
pub enum ChainError {
    #[error("tüm provider'lar başarısız: {0}")]
    AllFailed(String),
}

/// Sıralı fallback: 1F (info+token) → AD → DL → RD → TB → FREE. İlk başarı döner; hepsi başarısızsa Free.
pub async fn resolve_chain(
    client: &reqwest::Client,
    keys: &ApiKeys,
    url: &str,
) -> Result<ChainOutcome, ChainError> {
    // 1F
    if keys.has(Provider::OneFichier) {
        match onefichier_direct_url(client, &keys.onefichier, url).await {
            Ok((filename, _size, final_url)) => return Ok(ChainOutcome::Debrid(DebridSuccess { provider: Provider::OneFichier, filename, final_url })),
            Err(_) => {} // fallback
        }
    }
    // AllDebrid
    if keys.has(Provider::AllDebrid) {
        if let Ok(s) = alldebrid_unlock(client, &keys.alldebrid, url).await {
            return Ok(ChainOutcome::Debrid(s));
        }
    }
    // DebridLink
    if keys.has(Provider::DebridLink) {
        if let Ok(s) = debridlink_add(client, &keys.debridlink, url).await {
            return Ok(ChainOutcome::Debrid(s));
        }
    }
    // RealDebrid
    if keys.has(Provider::RealDebrid) {
        if let Ok(s) = realdebrid_unrestrict(client, &keys.realdebrid, url).await {
            return Ok(ChainOutcome::Debrid(s));
        }
    }
    // TorBox
    if keys.has(Provider::TorBox) {
        if let Ok(s) = torbox_webdl(client, &keys.torbox, url).await {
            return Ok(ChainOutcome::Debrid(s));
        }
    }
    // FREE fallback (her zaman var)
    Ok(ChainOutcome::Free)
}

// ---------------------------------------------------------------------------
// Faz4: OneFichier final_url indirme motoru OFD2→OF18 (HEAD atlama, varlık kontrol,
// 10x retry + 3 header variant + Range resume + AD 503 refresh + disk + .part
// + cancel + force_extract). Python OF11..OF18 parity.
// ---------------------------------------------------------------------------

use std::path::{Path, PathBuf};

/// 1Fichier final_url için header varyantları (OF11 Python `download_header_variants` parity).
/// 3 varyant: browser default → browser Accept:*/* → curl.
pub fn onefichier_header_variants() -> Vec<Vec<(String, String)>> {
    let base = crate::http::default_browser_headers(None);
    let v0 = base.clone();
    let mut v1 = base.clone();
    let mut found = false;
    for (k, v) in &mut v1 {
        if k.eq_ignore_ascii_case("Accept") { *v = "*/*".into(); found = true; }
    }
    if !found { v1.push(("Accept".into(), "*/*".into())); }
    let v2 = vec![
        ("User-Agent".into(), "curl/8.4.0".into()),
        ("Accept".into(), "*/*".into()),
        ("Accept-Encoding".into(), "identity".into()),
        ("Connection".into(), "keep-alive".into()),
    ];
    vec![v0, v1, v2]
}

/// AD/DL/RD için HEAD atlanmalı (geçici/tek kullanımlık URL — Python OFD2 parity).
pub fn should_skip_head_for_provider(provider: Provider) -> bool {
    matches!(provider, Provider::AllDebrid | Provider::DebridLink | Provider::RealDebrid)
}

/// HEAD ile remote_size al (transient provider'da None döner — atlama).
pub async fn head_remote_size(
    client: &reqwest::Client,
    final_url: &str,
    provider: Provider,
) -> Option<u64> {
    if should_skip_head_for_provider(provider) { return None; }
    let resp = client.head(final_url).send().await.ok()?;
    if !resp.status().is_success() { return None; }
    resp.headers().get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
}

/// Mevcut dosya durumu (OF10 parity).
#[derive(Debug, PartialEq, Eq)]
pub enum ExistingStatus {
    NotExists,
    ExistsAndMatches,
    ExistsButMismatch,
}

pub fn existing_file_status(dest_path: &Path, remote_size: Option<u64>) -> ExistingStatus {
    match std::fs::metadata(dest_path) {
        Ok(m) => {
            if let Some(rs) = remote_size {
                if m.len() == rs { ExistingStatus::ExistsAndMatches } else { ExistingStatus::ExistsButMismatch }
            } else {
                // lolroms gibi doğrulanamaz durumda varsa kabul (python H1l parity) — ama 1fichier için size biliniyorsa mismatch say
                ExistingStatus::ExistsAndMatches
            }
        }
        Err(_) => ExistingStatus::NotExists,
    }
}

/// Aynı taban adla farklı uzantıda dosya var mı? (OF8b/OF10 alternative parity). `None` → yok.
pub fn find_same_stem_existing(dest_dir: &Path, sanitized_filename: &str, remote_size: Option<u64>) -> Option<PathBuf> {
    let stem = Path::new(sanitized_filename).file_stem()?.to_str()?;
    let dir = std::fs::read_dir(dest_dir).ok()?;
    for entry in dir.flatten() {
        let p = entry.path();
        if !p.is_file() { continue; }
        let s = p.file_stem()?.to_str()?;
        if s == stem {
            // Boyut kontrolü
            if let Some(rs) = remote_size {
                if let Ok(m) = std::fs::metadata(&p) {
                    if m.len() != rs { continue; } // mismatch → indirme devam etmeli, bu dosyayı atlama
                }
            }
            if p.file_name()?.to_str()? != sanitized_filename {
                return Some(p);
            }
        }
    }
    None
}

/// `force_extract` kararı (OF17 parity) — `manager_core::extract::should_force_extract` delegesi.
pub fn decide_force_extract(
    is_zip_non_supported: bool,
    auto_extract: bool,
    platform_folder: &str,
    platform: &str,
) -> bool {
    manager_core::extract::should_force_extract(auto_extract, is_zip_non_supported, platform_folder, platform)
}

/// Faz4 ana indirme: final_url → dest_path, 10x retry, Range resume, AD 503 refresh, disk/precheck, .part→replace.
/// Python OF11..OF15 parity. `retry_delay` üretimde 10s, testte küçük tutulabilir.
pub async fn download_onefichier_final_url(
    client: &reqwest::Client,
    final_url: &str,
    dest_path: &Path,
    provider: Provider,
    api_keys: &ApiKeys,
    original_url: &str,
    cancel: Option<&crate::http::stream::CancelFlag>,
    on_progress: Option<std::sync::Arc<crate::http::stream::ProgressCb>>,
    max_retries: u32,
    retry_delay: Duration,
) -> Result<PathBuf, crate::http::DownloadError> {
    let dest_dir = dest_path.parent().unwrap_or(dest_path);
    // Yazılabilirlik pre-check (Gap-5 precheck_destination parity)
    match manager_core::disk::precheck_destination(dest_dir, 0) {
        Ok(()) => {}
        Err(manager_core::disk::DiskError::QueryFailed(_)) => {}
        Err(manager_core::disk::DiskError::PermissionDenied(m)) => return Err(crate::http::DownloadError::PermissionDenied(m)),
        Err(manager_core::disk::DiskError::InsufficientSpace { free, required }) => return Err(crate::http::DownloadError::InsufficientDiskSpace(format!("gerekli {required} bayt, mevcut {free} bayt"))),
    }

    let variants = onefichier_header_variants();
    let max = max_retries.max(1);
    let mut current_url = final_url.to_string();

    for attempt in 0..max {
        if cancel.map(|c| c.is_set()).unwrap_or(false) {
            return Err(crate::http::DownloadError::Canceled);
        }
        let resume = crate::http::stream::resume_offset(dest_path);
        let var_idx = (attempt as usize).min(variants.len().saturating_sub(1));
        let headers = &variants[var_idx];
        let mut builder = client.get(&current_url);
        for (k, v) in headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        if resume > 0 {
            builder = builder.header("Range", format!("bytes={resume}-"));
        }
        let resp = match builder.send().await {
            Ok(r) => r,
            Err(e) => {
                if attempt + 1 >= max {
                    return Err(crate::http::DownloadError::from(e));
                }
                tokio::time::sleep(retry_delay).await;
                continue;
            }
        };
        let status = resp.status().as_u16();
        // OF11a: AD 503 → refresh
        if status == 503 && provider == Provider::AllDebrid && attempt + 1 < max {
            if let Some((new_url, _)) = refresh_alldebrid_url(client, &api_keys.alldebrid, original_url).await {
                current_url = new_url;
            }
            tokio::time::sleep(retry_delay).await;
            continue;
        }
        if (500..=599).contains(&status) {
            if attempt + 1 >= max {
                return Err(crate::http::DownloadError::Http(format!("HTTP {status}")));
            }
            tokio::time::sleep(retry_delay).await;
            continue;
        }
        if !(200..=299).contains(&status) && status != 206 {
            return Err(crate::http::DownloadError::Http(format!("HTTP {status}")));
        }
        // Başarılı yanıt → disk alan kontrolü (total_size)
        let content_len = resp.headers().get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let range_total = resp.headers().get(reqwest::header::CONTENT_RANGE)
            .and_then(|v| v.to_str().ok()).and_then(crate::http::guards::parse_content_range_total);
        let total = if let Some(t) = range_total { t } else if content_len > 0 { content_len + if status == 206 { resume } else { 0 } } else { 0 };
        if total > 0 {
            match manager_core::disk::precheck_destination(dest_dir, total) {
                Ok(()) => {}
                Err(manager_core::disk::DiskError::QueryFailed(_)) => {}
                Err(manager_core::disk::DiskError::PermissionDenied(m)) => return Err(crate::http::DownloadError::PermissionDenied(m)),
                Err(manager_core::disk::DiskError::InsufficientSpace { free, required }) => return Err(crate::http::DownloadError::InsufficientDiskSpace(format!("gerekli {required} bayt, mevcut {free} bayt"))),
            }
        }
        let (s, _detect) = crate::http::stream::download_stream_async(resp, dest_path, resume, cancel, on_progress.clone()).await
            .map_err(crate::http::DownloadError::from)?;
        if s.canceled {
            let _ = tokio::fs::remove_file(crate::http::stream::part_path_for(dest_path)).await;
            return Err(crate::http::DownloadError::Canceled);
        }
        if s.downloaded == 0 {
            let _ = tokio::fs::remove_file(dest_path).await;
            if attempt + 1 >= max {
                return Err(crate::http::DownloadError::EmptyResponse("0 byte".into()));
            }
            tokio::time::sleep(retry_delay).await;
            continue;
        }
        crate::http::stream::finalize_part(dest_path, s.downloaded).await
            .map_err(crate::http::DownloadError::from)?;
        return Ok(dest_path.to_path_buf());
    }
    Err(crate::http::DownloadError::Http("tüm denemeler başarısız".into()))
}

// ---------------------------------------------------------------------------
// Faz5: FREE tam akış (OFF) + orchestrator finalize
// Python `download_1fichier_free_mode` parity: GET→wait→f1 POST (3x retry)→
// candidate HEAD/GET doğrulama→HEAD filename→stream .part→finalize.
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum FreeModeError {
    #[error("iptal edildi")]
    Canceled,
    #[error("bloklandı: {0}")]
    Blocked(String),
    #[error("indirme linki bulunamadı")]
    NotFound,
    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("ağ hatası: {0}")]
    Network(String),
    #[error("boş yanıt")]
    Empty,
    #[error("io: {0}")]
    Io(String),
}

fn extract_cd_filename(cd: &str) -> Option<String> {
    // filename*=UTF-8'' veya filename="..." parity (vimm.rs re_cd_filename ile aynı mantık)
    let re = Regex::new(r#"(?i)filename\*?=(?:UTF-8''|"|'|)([^"';\r\n]+)"#).ok()?;
    let caps = re.captures(cd)?;
    let raw = caps.get(1)?.as_str().trim().trim_matches('"').trim_matches('\'').to_string();
    let decoded = percent_encoding::percent_decode_str(&raw).decode_utf8().map(|c| c.into_owned()).unwrap_or(raw);
    // percent_decode + sanitize
    let s = decoded.trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

async fn validate_free_candidate(
    client: &reqwest::Client,
    candidate: &str,
) -> bool {
    // HEAD önce
    if let Ok(resp) = client.head(candidate).send().await {
        let status = resp.status().as_u16();
        if status < 400 {
            if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
                if ct.to_ascii_lowercase().contains("text/html") {
                    // HTML ise GET fallback dene
                } else {
                    return true;
                }
            } else {
                return true;
            }
        } else {
            return false;
        }
    }
    // HEAD başarısız veya HTML → hızlı GET ile doğrula (body preview)
    if let Ok(resp) = client.get(candidate).send().await {
        let status = resp.status().as_u16();
        if status >= 400 { return false; }
        if let Some(ct) = resp.headers().get(reqwest::header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) {
            if ct.to_ascii_lowercase().contains("text/html") {
                // body'de <html var mı kontrol et (landing page)
                if let Ok(txt) = resp.text().await {
                    if txt.to_ascii_lowercase().contains("<html") { return false; }
                } else { return false; }
            }
        }
        return true;
    }
    false
}

/// FREE tam indirme (OFF). `dest_dir` altına gerçek filename ile kaydeder.
/// Python parity: wait_callback/progress_callback/cancel_event desteklenir (opsiyonel).
pub async fn free_mode_download(
    client: &reqwest::Client,
    url: &str,
    dest_dir: &Path,
    cancel: Option<&crate::http::stream::CancelFlag>,
    on_progress: Option<std::sync::Arc<crate::http::stream::ProgressCb>>,
    on_wait: Option<std::sync::Arc<dyn Fn(u64, u64) + Send + Sync + 'static>>,
) -> Result<PathBuf, FreeModeError> {
    if cancel.map(|c| c.is_set()).unwrap_or(false) { return Err(FreeModeError::Canceled); }
    tokio::fs::create_dir_all(dest_dir).await.map_err(|e| FreeModeError::Io(e.to_string()))?;

    // 1. GET page initial
    let resp = client.get(url).send().await.map_err(|e| FreeModeError::Network(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(FreeModeError::Http { status: resp.status().as_u16(), message: format!("GET {}", resp.status()) });
    }
    let page_url = resp.url().to_string();
    let html = resp.text().await.map_err(|e| FreeModeError::Network(e.to_string()))?;

    // 2. wait countdown
    let wait_s = extract_wait_seconds(&html);
    if wait_s > 0 {
        for remaining in (1..=wait_s).rev() {
            if cancel.map(|c| c.is_set()).unwrap_or(false) { return Err(FreeModeError::Canceled); }
            if let Some(cb) = &on_wait { cb(remaining, wait_s); }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }

    // 3. form f1 POST (3x retry, extra wait handling)
    let mut final_html = html.clone();
    let mut final_page_url = page_url.clone();
    if let Some(data) = parse_free_form_data(&html) {
        let origin = url::Url::parse(&page_url).ok()
            .and_then(|u| Some(format!("{}://{}", u.scheme(), u.host_str()?)))
            .unwrap_or_else(|| page_url.clone());
        let mut post_html: Option<String> = None;
        let mut post_page_url = page_url.clone();
        for _ in 0..3 {
            if cancel.map(|c| c.is_set()).unwrap_or(false) { return Err(FreeModeError::Canceled); }
            let post_resp = client.post(&post_page_url)
                .header("Referer", post_page_url.clone())
                .header("Origin", origin.clone())
                .form(&data)
                .send().await.map_err(|e| FreeModeError::Network(e.to_string()))?;
            let status = post_resp.status().as_u16();
            if !(200..=299).contains(&status) && status != 303 {
                // 3xx redirect zaten reqwest follow eder; diğer hata → retry
                tokio::time::sleep(Duration::from_secs(1)).await;
                continue;
            }
            let purl = post_resp.url().to_string();
            let h = post_resp.text().await.map_err(|e| FreeModeError::Network(e.to_string()))?;
            // extra wait?
            let extra = extract_wait_seconds(&h);
            if extra > 0 {
                for remaining in (1..=extra).rev() {
                    if cancel.map(|c| c.is_set()).unwrap_or(false) { return Err(FreeModeError::Canceled); }
                    if let Some(cb) = &on_wait { cb(remaining, extra); }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                post_page_url = purl.clone();
                continue;
            }
            post_html = Some(h);
            post_page_url = purl;
            break;
        }
        if let Some(h) = post_html {
            final_html = h;
            final_page_url = post_page_url;
        } else {
            return Err(FreeModeError::Http { status: 0, message: "form POST başarısız".into() });
        }
        if let Some(block) = extract_free_block_reason(&final_html) {
            return Err(FreeModeError::Blocked(block));
        }
    }

    // 4. candidate extraction + HEAD/GET validation
    let candidates = extract_free_candidates(&final_html, &final_page_url);
    let mut direct_link: Option<String> = None;
    for cand in &candidates {
        if validate_free_candidate(client, cand).await {
            direct_link = Some(cand.clone());
            break;
        }
    }
    let dl = match direct_link {
        Some(u) => u,
        None => {
            if let Some(block) = extract_free_block_reason(&final_html) {
                return Err(FreeModeError::Blocked(block));
            }
            return Err(FreeModeError::NotFound);
        }
    };

    // 5. HEAD filename
    let head = client.head(&dl).send().await.map_err(|e| FreeModeError::Network(e.to_string()))?;
    let mut filename = "downloaded_file".to_string();
    if let Some(cd) = head.headers().get(reqwest::header::CONTENT_DISPOSITION).and_then(|v| v.to_str().ok()) {
        if let Some(f) = extract_cd_filename(cd) { filename = f; }
    }
    // fallback: URL son segment
    if filename == "downloaded_file" {
        if let Some(seg) = dl.split('/').filter(|s| !s.is_empty()).last() {
            if !seg.is_empty() && !seg.contains('?') { filename = seg.to_string(); }
        }
    }
    filename = sanitize_filename(&filename);
    // percent decode for URL encoded
    if let Ok(dec) = percent_encoding::percent_decode_str(&filename).decode_utf8() { filename = dec.into_owned(); }
    let dest_path = dest_dir.join(&filename);

    // 6. stream download (FREE python directly writes to dest, but we use .part parity for resume safety)
    let resp = client.get(&dl).send().await.map_err(|e| FreeModeError::Network(e.to_string()))?;
    let status = resp.status().as_u16();
    if !(200..=299).contains(&status) && status != 206 {
        return Err(FreeModeError::Http { status, message: format!("GET {status}") });
    }
    let total = resp.headers().get(reqwest::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    if total > 0 {
        match manager_core::disk::precheck_destination(dest_dir, total) {
            Ok(()) => {}
            Err(manager_core::disk::DiskError::QueryFailed(_)) => {}
            Err(e) => return Err(FreeModeError::Io(e.to_string())),
        }
    }
    // resume? FREE modda .part kullanımı yoktu ama Faz4 parity için destekle: resume 0
    let (s, _detect) = crate::http::stream::download_stream_async(resp, &dest_path, 0, cancel, on_progress).await
        .map_err(|e| FreeModeError::Io(e.to_string()))?;
    if s.canceled { return Err(FreeModeError::Canceled); }
    if s.downloaded == 0 { return Err(FreeModeError::Empty); }
    crate::http::stream::finalize_part(&dest_path, s.downloaded).await.map_err(|e| FreeModeError::Io(e.to_string()))?;
    Ok(dest_path)
}

/// Orchestrator: 1Fichier URL → chain → download (debrid veya FREE) → force_extract → provider history.
/// Python `download_from_1fichier` thread ana akışı parity (OF2..OF18 + OFF).
pub struct OneFichierOrchestrator {
    pub client: reqwest::Client,
    pub keys: ApiKeys,
}

impl OneFichierOrchestrator {
    pub fn new(keys: ApiKeys) -> Self {
        let client = reqwest::Client::builder().cookie_store(true).build().unwrap_or_else(|_| reqwest::Client::new());
        Self { client, keys }
    }
    pub fn with_client(client: reqwest::Client, keys: ApiKeys) -> Self {
        Self { client, keys }
    }

    /// Tam zincir: resolve → download → extract (opsiyonel). Başarıda (provider, dest_path) döner.
    pub async fn download(
        &self,
        url: &str,
        dest_dir: &Path,
        game_name: &str,
        platform: &str,
        is_zip_non_supported: bool,
        auto_extract: bool,
        cancel: Option<&crate::http::stream::CancelFlag>,
        on_progress: Option<std::sync::Arc<crate::http::stream::ProgressCb>>,
    ) -> Result<(Provider, PathBuf), crate::http::DownloadError> {
        // Dedup? caller yönetir (DedupCache)
        let chain = resolve_chain(&self.client, &self.keys, url).await
            .map_err(|e| crate::http::DownloadError::Client(e.to_string()))?;
        let (provider, final_url, filename) = match chain {
            ChainOutcome::Debrid(s) => (s.provider, s.final_url.clone(), if s.filename.is_empty() { game_name.to_string() } else { s.filename.clone() }),
            ChainOutcome::Free => {
                // FREE scrape
                let p = free_mode_download(&self.client, url, dest_dir, cancel, on_progress.clone(), None).await
                    .map_err(|e| match e {
                        FreeModeError::Canceled => crate::http::DownloadError::Canceled,
                        FreeModeError::Blocked(m) => crate::http::DownloadError::Http(m),
                        FreeModeError::NotFound => crate::http::DownloadError::Http("Lien de téléchargement introuvable".into()),
                        FreeModeError::Http { status, message } => crate::http::DownloadError::Http(format!("FREE HTTP {status}: {message}")),
                        FreeModeError::Network(m) => crate::http::DownloadError::Network(m),
                        FreeModeError::Empty => crate::http::DownloadError::EmptyResponse("FREE 0 byte".into()),
                        FreeModeError::Io(m) => crate::http::DownloadError::Client(m),
                    })?;
                // FREE'de filename zaten dest içinde, provider FREE
                let prov = Provider::Free;
                // force_extract + chmod
                let need_extract = decide_force_extract(is_zip_non_supported, auto_extract, &platform_folder_hint(platform), platform);
                if need_extract {
                    // postprocess (extract) — manager_core::extract::extract_archive
                    let _ = extract_after_download(&p, dest_dir, platform).await;
                } else {
                    #[cfg(unix)] { let _ = tokio::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o644)).await; }
                }
                return Ok((prov, p));
            }
        };
        // Debrid yolu: dest_path kur
        let sanitized = sanitize_filename(&filename);
        let dest_path = dest_dir.join(sanitized);
        // BIOS redirect? caller dest_dir zaten platform'a göre seçili; orchestrator dest_dir'i doğrudan kullanır (redirect_bios_dest api.rs'de yapılır)
        // Varlık kontrolü (OF10)
        let head_size = head_remote_size(&self.client, &final_url, provider).await;
        match existing_file_status(&dest_path, head_size) {
            ExistingStatus::ExistsAndMatches => return Ok((provider, dest_path)),
            ExistingStatus::ExistsButMismatch => { let _ = tokio::fs::remove_file(&dest_path).await; }
            ExistingStatus::NotExists => {}
        }
        // alternatif uzantı kontrolü
        if let Some(alt) = find_same_stem_existing(dest_dir, &dest_path.file_name().unwrap().to_string_lossy(), head_size) {
            return Ok((provider, alt));
        }
        // indir
        let out = download_onefichier_final_url(&self.client, &final_url, &dest_path, provider, &self.keys, url, cancel, on_progress, 10, Duration::from_secs(10)).await?;
        // force_extract & chmod
        let pf = dest_path.parent().and_then(|p| p.file_name()).and_then(|s| s.to_str()).unwrap_or("");
        let need_extract = decide_force_extract(is_zip_non_supported, auto_extract, pf, platform);
        if need_extract {
            let _ = extract_after_download(&out, dest_dir, platform).await;
        } else {
            #[cfg(unix)] { let _ = tokio::fs::set_permissions(&out, std::os::unix::fs::PermissionsExt::from_mode(0o644)).await; }
        }
        // provider history caller tarafından history_provider_fields ile yazılır
        let _ = dest_path; // silence
        Ok((provider, out))
    }
}

fn platform_folder_hint(platform: &str) -> String {
    platform.to_ascii_lowercase().replace(' ', "")
}

async fn extract_after_download(path: &Path, dest_dir: &Path, _platform: &str) -> Result<(), String> {
    let ext = path.extension().and_then(|e| e.to_str()).map(|s| s.to_ascii_lowercase()).unwrap_or_default();
    let res = match ext.as_str() {
        "zip" => manager_core::extract::extract_archive(path, dest_dir).map(|_| ()),
        "7z" => manager_core::extract::extract_archive(path, dest_dir).map(|_| ()),
        "rar" => manager_core::extract::extract_archive(path, dest_dir).map(|_| ()),
        _ => return Ok(()),
    };
    res.map_err(|e| e.to_string())
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

    // --- Faz3 pure helpers ---

    #[test]
    fn debridlink_map() {
        assert_eq!(debridlink_friendly("badToken"), "DL: Invalid API key");
        assert_eq!(debridlink_friendly("maxLink"), "DL: Daily link limit reached");
        assert_eq!(debridlink_friendly("unknownXYZ"), "DL: unknownXYZ");
    }

    #[test]
    fn realdebrid_map() {
        assert_eq!(realdebrid_friendly(4, None), "RD: File not found");
        assert_eq!(realdebrid_friendly(20, None), "RD: Premium account only");
        assert_eq!(realdebrid_friendly(20, Some("hoster_not_free")), "RD: Premium account only");
        assert_eq!(realdebrid_friendly(999, Some("custom err")), "RD: custom err");
    }

    #[test]
    fn torbox_map() {
        assert_eq!(torbox_friendly("BAD_TOKEN", None), "TB: Invalid API key");
        assert_eq!(torbox_friendly("LINK_OFFLINE", None), "TB: Link offline or inaccessible");
        assert_eq!(torbox_friendly("CUSTOM", Some("detail text")), "TB: detail text");
    }

    #[test]
    fn md5_hex_known() {
        assert_eq!(md5_hex("hello"), "5d41402abc4b2a76b9719d911017c592");
        assert_eq!(md5_hex(""), "d41d8cd98f00b204e9800998ecf8427e");
    }

    #[test]
    fn chain_available_order() {
        let keys = ApiKeys { onefichier: "".into(), alldebrid: "a".into(), debridlink: "".into(), realdebrid: "r".into(), torbox: "".into() };
        let avail = keys.available_providers();
        assert_eq!(avail, vec![Provider::AllDebrid, Provider::RealDebrid, Provider::Free]);
    }

    // --- Faz4 pure helpers ---

    #[test]
    fn header_variants_shape() {
        let v = onefichier_header_variants();
        assert_eq!(v.len(), 3);
        // v0 browser UA, v1 Accept */*, v2 curl
        assert!(v[0].iter().any(|(k,_)| k == "User-Agent"));
        assert!(v[1].iter().any(|(k, val)| k == "Accept" && val == "*/*"));
        assert!(v[2].iter().any(|(k,val)| k == "User-Agent" && val == "curl/8.4.0"));
    }

    #[test]
    fn skip_head_for_provider() {
        assert!(should_skip_head_for_provider(Provider::AllDebrid));
        assert!(should_skip_head_for_provider(Provider::DebridLink));
        assert!(should_skip_head_for_provider(Provider::RealDebrid));
        assert!(!should_skip_head_for_provider(Provider::OneFichier));
        assert!(!should_skip_head_for_provider(Provider::TorBox));
    }

    #[test]
    fn existing_status_matches() {
        let dir = std::env::temp_dir().join(format!("rgsx-of4-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("file.bin");
        std::fs::write(&p, b"12345").unwrap();
        assert_eq!(existing_file_status(&p, Some(5)), ExistingStatus::ExistsAndMatches);
        assert_eq!(existing_file_status(&p, Some(99)), ExistingStatus::ExistsButMismatch);
        assert_eq!(existing_file_status(&dir.join("nope"), Some(5)), ExistingStatus::NotExists);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_stem_existing() {
        let dir = std::env::temp_dir().join(format!("rgsx-of4-stem-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("game.zip"), b"aaaa").unwrap();
        std::fs::write(dir.join("game.rar"), b"aaaa").unwrap();
        // search for game.zip -> should find game.rar as alternative (same stem, different ext)
        let alt = find_same_stem_existing(&dir, "game.zip", Some(4));
        assert!(alt.is_some());
        let alt_path = alt.unwrap();
        assert!(alt_path.file_name().unwrap().to_string_lossy().ends_with(".rar"));
        // size mismatch -> should not return
        let alt2 = find_same_stem_existing(&dir, "game.zip", Some(999));
        assert!(alt2.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decide_force_extract_cases() {
        // zip non-supported + auto => true
        assert!(decide_force_extract(true, true, "snes", "Super Nintendo"));
        // normal zip + auto false => false
        assert!(!decide_force_extract(false, false, "snes", "snes"));
        // PS3 redump => force (auto'dan bağımsız)
        assert!(decide_force_extract(false, true, "ps3", "ps3"));
        assert!(decide_force_extract(false, false, "ps3", "ps3"));
        assert!(!decide_force_extract(false, true, "snes", "snes"));
        // BIOS + auto => force
        assert!(decide_force_extract(false, true, "bios", "BIOS"));
        assert!(!decide_force_extract(false, false, "bios", "BIOS"));
    }

    // --- Faz5 FREE helpers ---

    #[test]
    fn cd_filename_extract() {
        assert_eq!(extract_cd_filename("attachment; filename=\"game.zip\"").as_deref(), Some("game.zip"));
        assert_eq!(extract_cd_filename("attachment; filename*=UTF-8''game%20test.zip").as_deref(), Some("game test.zip"));
        assert!(extract_cd_filename("inline; no-filename").is_none());
    }

    #[test]
    fn free_block_reason_still_works() {
        let html = "<div>Le téléchargement gratuit est temporairement limité. Veuillez vous identifiez-vous immediatement</div>";
        assert!(extract_free_block_reason(html).is_some());
        assert!(extract_free_block_reason("<p>ok</p>").is_none());
    }
}
