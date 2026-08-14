//! Faz 10c/3/2 — katalog route'ları için `CatalogSource` trait + Python proxy.
//!
//! Strangler/atest misali: Rust `manager-http` katalog handler'ları (`platforms`,
//! `search`, `games`, `translations`, `image`), gerçek mantığı Python'da bırakıp
//! onu `127.0.0.1:5000` (veya `RGSX_PYTHON_MANAGER_URL`) üzerinden proxy'ler.
//! Bu, contract'ı birebir korur (Python yanıtı aynen iletilir) ve native Rust
//! portunu (dış ROM kaynak istemcileri) ayrı bir alt faz'a erteletir. `catalog`
//! `None` ise handler'lar mevcut placeholder davranışına düşer (geriye uyumlu).

use async_trait::async_trait;
use serde_json::Value;

/// Katalog kaynağı hatası (proxy çökmesi → handler placeholder'a düşer).
#[derive(Debug)]
pub struct CatalogError(pub String);

/// Katalog veri kaynağı — test'te `FakeCatalog` ile enjekte edilebilir.
#[async_trait]
pub trait CatalogSource: Send + Sync {
    /// JSON dönen GET route'u proxy'ler (ör. `/api/platforms`, `/api/search?q=zelda`).
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError>;
    /// JSON dönen POST route'u proxy'ler (gövde iletilir).
    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError>;
    /// İkili (zip) POST route'u proxy'ler (ham bayt + content-type).
    async fn post_binary(&self, route: &str, body: &Value) -> Result<(Vec<u8>, String), CatalogError>;
    /// Box-art görselini (ham bayt + content-type) proxy'ler.
    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError>;
}

/// Python `ManagerHandler` (HTTP port `RGSX_PYTHON_MANAGER_URL`) proxy'si.
#[derive(Clone)]
pub struct PythonCatalog {
    base: String,
    client: reqwest::Client,
}

impl PythonCatalog {
    pub fn new(base: String) -> Self {
        Self {
            base,
            client: reqwest::Client::new(),
        }
    }
}

fn encode(seg: &str) -> String {
    percent_encoding::utf8_percent_encode(seg, percent_encoding::NON_ALPHANUMERIC).to_string()
}

#[async_trait]
impl CatalogSource for PythonCatalog {
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError> {
        let url = format!("{}{}", self.base, route);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok(v)
    }

    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError> {
        let url = format!("{}/api/image/{}", self.base, encode(platform));
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/png")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok((bytes.to_vec(), ct))
    }

    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError> {
        let url = format!("{}{}", self.base, route);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let v: Value = resp
            .json()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok(v)
    }

    async fn post_binary(&self, route: &str, body: &Value) -> Result<(Vec<u8>, String), CatalogError> {
        let url = format!("{}{}", self.base, route);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        let ct = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| CatalogError(e.to_string()))?;
        Ok((bytes.to_vec(), ct))
    }
}

// ===========================================================================
// Faz 12c — NativeCatalog: Python'sız, local dosyalardan catalog üretimi.
//
// Veri kaynağı Python `get_cached_sources()`/`load_sources()`/`load_games()`
// ile birebir aynı dosyalardır (systems_list.json, games/<platform>.json,
// languages/<lang>.json, images/<platform>.*); böylece çıktı şekli Python ile
// aynı kalır ve offline çalışır. Komut POST'ları (download/queue/cancel) native
// değildir → opsiyonel `python` fallback'e proxy edilir (Faz 12e'ye kaldı).
// ===========================================================================

use std::path::{Path, PathBuf};

/// Box-art uzantısı → content-type.
fn image_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// `%XX` decode (query/platform segment çözümü).
fn pct_decode(s: &str) -> String {
    percent_encoding::percent_decode_str(s)
        .decode_utf8_lossy()
        .to_string()
}

/// Basit query parse (`q=zelda&x=1` → map).
fn parse_query(q: &str) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for pair in q.split('&') {
        if let Some((k, v)) = pair.split_once('=') {
            out.insert(k.to_string(), pct_decode(v));
        }
    }
    out
}

/// BIOS platform adları — ROM klasörü yokken bile görünür.
const BIOS_NAMES: &[&str] = &["- BIOS by TMCTV -", "- BIOS"];

#[derive(Clone)]
pub struct NativeCatalog {
    sources_file: PathBuf,
    games_folder: PathBuf,
    images_folder: PathBuf,
    languages_folder: PathBuf,
    roms_folder: Option<PathBuf>,
    show_unsupported: bool,
    default_language: String,
    /// Komut POST'ları için Python fallback (download/queue/cancel...).
    python: Option<PythonCatalog>,
}

impl NativeCatalog {
    /// `RGSX_NATIVE_CATALOG=1` ile main.rs'ten kurulur. Yollar `RGSX_DATA_DIR`
    /// altından türetilir (Python `SAVE_FOLDER` eşleniği); tek tek override edilebilir.
    pub fn from_env() -> Self {
        let data_dir = std::env::var("RGSX_DATA_DIR").unwrap_or_else(|_| ".".to_string());
        let d = PathBuf::from(data_dir);
        let sources_file = env_path("RGSX_SOURCES_FILE", d.join("systems_list.json"));
        let games_folder = env_path("RGSX_GAMES_FOLDER", d.join("games"));
        let images_folder = env_path("RGSX_IMAGES_FOLDER", d.join("images"));
        let languages_folder = env_path("RGSX_LANGUAGES_FOLDER", d.join("languages"));
        let roms_folder = std::env::var("RGSX_ROMS_FOLDER")
            .ok()
            .filter(|s| !s.is_empty())
            .map(PathBuf::from);
        let show_unsupported = std::env::var("RGSX_SHOW_UNSUPPORTED")
            .map(|v| v == "1")
            .unwrap_or(true);
        let default_language = std::env::var("RGSX_LANGUAGE").unwrap_or_else(|_| "en".to_string());
        let python = std::env::var("RGSX_PYTHON_MANAGER_URL")
            .ok()
            .filter(|u| !u.is_empty())
            .map(PythonCatalog::new);
        Self {
            sources_file,
            games_folder,
            images_folder,
            languages_folder,
            roms_folder,
            show_unsupported,
            default_language,
            python,
        }
    }

    /// `systems_list.json` → normalize + runtime game-file filtresi (Python `load_sources`).
    fn load_sources(&self) -> Vec<Value> {
        let mut sources: Vec<Value> = match std::fs::read_to_string(&self.sources_file) {
            Ok(txt) => serde_json::from_str::<Value>(&txt)
                .ok()
                .and_then(|v| v.as_array().cloned())
                .unwrap_or_default(),
            Err(_) => vec![],
        };

        // Anahtar normalizasyonu (legacy system_image/dossier).
        let mut normalized = Vec::new();
        for raw in sources.drain(..) {
            if let Value::Object(mut m) = raw {
                if !m.contains_key("platform_image") {
                    let legacy = m.remove("system_image").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default();
                    m.insert("platform_image".into(), Value::String(legacy));
                }
                if !m.contains_key("folder") {
                    if let Some(f) = m.get("dossier").and_then(|v| v.as_str()).map(str::to_string) {
                        m.insert("folder".into(), Value::String(f));
                    }
                }
                normalized.push(Value::Object(m));
            }
        }
        sources = normalized;

        // Runtime: yalnızca games/<name>.json dosyası olanları tut.
        let existing_files: std::collections::HashSet<String> = if self.games_folder.is_dir() {
            std::fs::read_dir(&self.games_folder)
                .map(|e| {
                    e.flatten()
                        .filter_map(|f| f.file_name().to_str().map(|s| s.to_ascii_lowercase()))
                        .filter(|s| s.ends_with(".json"))
                        .map(|s| s.trim_end_matches(".json").to_string())
                        .collect()
                })
                .unwrap_or_default()
        } else {
            std::collections::HashSet::new()
        };

        sources
            .into_iter()
            .filter(|s| {
                let name = s.get("platform_name").and_then(|v| v.as_str()).unwrap_or("");
                name.is_empty() || existing_files.contains(&name.to_ascii_lowercase())
            })
            .collect()
    }

    /// `games/<platform>.json` → [{name,url,size}] (Python `load_games` sadeleştirilmiş).
    fn load_games(&self, platform: &str) -> Vec<(String, Option<String>, Option<String>)> {
        let file = self.games_folder.join(format!("{platform}.json"));
        let data: Value = match std::fs::read_to_string(&file) {
            Ok(txt) => serde_json::from_str(&txt).unwrap_or(Value::Null),
            Err(_) => return vec![],
        };
        let arr = match &data {
            Value::Array(a) => a.clone(),
            Value::Object(m) if m.contains_key("games") => {
                m.get("games").and_then(|v| v.as_array()).cloned().unwrap_or_default()
            }
            _ => return vec![],
        };

        let mut out = Vec::new();
        for item in arr {
            match &item {
                Value::Array(t) => {
                    if t.is_empty() {
                        continue;
                    }
                    let name = t.first().and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let url = t.get(1).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string);
                    let size = t.get(2).and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string);
                    if !name.is_empty() {
                        out.push((name, url, size));
                    }
                }
                Value::Object(m) => {
                    let name = m
                        .get("game_name").or_else(|| m.get("name")).or_else(|| m.get("title")).or_else(|| m.get("game"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let url = m
                        .get("url").or_else(|| m.get("download")).or_else(|| m.get("link")).or_else(|| m.get("href"))
                        .and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string);
                    let size = m
                        .get("size").or_else(|| m.get("filesize")).or_else(|| m.get("length"))
                        .and_then(|v| v.as_str()).filter(|s| !s.trim().is_empty()).map(str::to_string);
                    out.push((name, url, size));
                }
                _ => {}
            }
        }
        out
    }

    /// Gizli/platform filtresi (Python `_api_platforms` mantığı, sadeleştirilmiş).
    fn is_hidden(&self, name: &str, folder: &str) -> bool {
        if self.show_unsupported {
            return false;
        }
        if BIOS_NAMES.contains(&name) {
            return false;
        }
        if let Some(roms) = &self.roms_folder {
            if !folder.is_empty() && !roms.join(folder).is_dir() {
                return true;
            }
        }
        false
    }

    fn build_platforms(&self) -> Value {
        let sources = self.load_sources();
        let mut platforms = Vec::new();
        for s in &sources {
            let name = s.get("platform_name").and_then(|v| v.as_str()).unwrap_or("");
            let folder = s.get("folder").and_then(|v| v.as_str()).unwrap_or("");
            if self.is_hidden(name, folder) {
                continue;
            }
            let mut p = s.clone();
            let games_count = self.load_games(name).len();
            if let Value::Object(m) = &mut p {
                m.insert("games_count".into(), Value::from(games_count as u64));
            }
            platforms.push(p);
        }
        serde_json::json!({ "success": true, "count": platforms.len(), "platforms": platforms })
    }

    fn build_search(&self, q: &str) -> Value {
        let term = q.to_lowercase();
        let words: Vec<&str> = term.split_whitespace().collect();
        if term.trim().is_empty() {
            return serde_json::json!({ "success": true, "search_term": "", "results": { "platforms": [], "games": [] } });
        }
        let sources = self.load_sources();
        let mut matching_platforms = Vec::new();
        let mut matching_games = Vec::new();
        for s in &sources {
            let name = s.get("platform_name").and_then(|v| v.as_str()).unwrap_or("");
            let folder = s.get("folder").and_then(|v| v.as_str()).unwrap_or("");
            if self.is_hidden(name, folder) {
                continue;
            }
            if name.to_lowercase().contains(&term) {
                matching_platforms.push(serde_json::json!({
                    "platform_name": name,
                    "folder": folder,
                    "platform_image": s.get("platform_image").and_then(|v| v.as_str()).unwrap_or(""),
                    "games_count": self.load_games(name).len(),
                }));
            }
            for (gname, url, size) in self.load_games(name) {
                let gl = gname.to_lowercase();
                if words.iter().all(|w| gl.contains(w)) {
                    matching_games.push(serde_json::json!({
                        "game_name": gname,
                        "platform": name,
                        "url": url,
                        "size": size,
                        "downloaded": false,
                    }));
                }
            }
        }
        serde_json::json!({
            "success": true,
            "search_term": q,
            "results": { "platforms": matching_platforms, "games": matching_games },
        })
    }

    fn build_games(&self, platform: &str) -> Value {
        let games: Vec<Value> = self
            .load_games(platform)
            .into_iter()
            .map(|(name, url, size)| {
                serde_json::json!({ "name": name, "url": url, "size": size, "downloaded": false })
            })
            .collect();
        serde_json::json!({
            "success": true,
            "platform": platform,
            "count": games.len(),
            "games": games,
        })
    }

    fn build_translations(&self) -> Value {
        let lang = &self.default_language;
        let file = self.languages_folder.join(format!("{lang}.json"));
        let translations = match std::fs::read_to_string(&file) {
            Ok(txt) => serde_json::from_str::<Value>(&txt).unwrap_or(Value::Object(Default::default())),
            Err(_) => Value::Object(Default::default()),
        };
        let mut t = match translations {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        t.insert("_language".into(), Value::String(lang.clone()));
        serde_json::json!({ "success": true, "language": lang, "translations": Value::Object(t) })
    }

    fn read_image(&self, platform: &str) -> Option<(Vec<u8>, String)> {
        // Aday taban adlar: (1) doğrudan argüman — scraper platform_name ile
        // adlandırdıysa; (2) platform_name -> platform_image eşlemesi (OTA layout:
        // images/3do.png). Her ikisini de deneriz ki fetch mekanizmasından bağımsız
        // çalışsın.
        let mut candidates: Vec<String> = vec![platform.to_string()];
        if let Some(src) = self
            .load_sources()
            .into_iter()
            .find(|s| s.get("platform_name").and_then(|v| v.as_str()) == Some(platform))
        {
            if let Some(img) = src.get("platform_image").and_then(|v| v.as_str()) {
                let base = img
                    .rsplit_once('.')
                    .map(|(b, _)| b.to_string())
                    .unwrap_or_else(|| img.to_string());
                candidates.push(base);
            }
        }
        for cand in &candidates {
            for ext in ["png", "jpg", "jpeg", "webp", "gif", "svg"] {
                let path = self.images_folder.join(format!("{cand}.{ext}"));
                if let Ok(bytes) = std::fs::read(&path) {
                    return Some((bytes, image_content_type(&path).to_string()));
                }
            }
        }
        None
    }
}

#[async_trait]
impl CatalogSource for NativeCatalog {
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError> {
        if route.starts_with("/api/platforms") {
            return Ok(self.build_platforms());
        }
        if route.starts_with("/api/search") {
            let q = route.split_once('?').map(|(_, q)| parse_query(q)).and_then(|m| m.get("q").cloned()).unwrap_or_default();
            return Ok(self.build_search(&q));
        }
        if route.starts_with("/api/games/") {
            let platform = pct_decode(route.trim_start_matches("/api/games/"));
            return Ok(self.build_games(&platform));
        }
        if route.starts_with("/api/translations") {
            return Ok(self.build_translations());
        }
        // Bilinmeyen GET route → Python fallback (varsa).
        if let Some(p) = &self.python {
            return p.get_json(route).await;
        }
        Err(CatalogError(format!("native catalog desteklemiyor: {route}")))
    }

    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError> {
        if let Some(p) = &self.python {
            return p.post_json(route, body).await;
        }
        Err(CatalogError(format!("native catalog POST desteklemiyor: {route}")))
    }

    async fn post_binary(&self, route: &str, body: &Value) -> Result<(Vec<u8>, String), CatalogError> {
        if let Some(p) = &self.python {
            return p.post_binary(route, body).await;
        }
        Err(CatalogError(format!("native catalog POST desteklemiyor: {route}")))
    }

    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError> {
        if let Some(img) = self.read_image(platform) {
            return Ok(img);
        }
        if let Some(p) = &self.python {
            return p.get_image(platform).await;
        }
        Err(CatalogError(format!("image bulunamadı: {platform}")))
    }
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var(key).ok().filter(|s| !s.is_empty()).map(PathBuf::from).unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, content: impl AsRef<[u8]>) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content.as_ref()).unwrap();
    }

    fn fixture() -> (tempfile::TempDir, NativeCatalog) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(&root.join("systems_list.json"),
            r#"[{"platform_name":"NES","folder":"nes","platform_image":"nes.png"},{"platform_name":"SNES","folder":"snes"}]"#);
        write(&root.join("games").join("NES.json"),
            r#"[["Super Mario Bros","http://x/mario.zip","1.2M"],{"game_name":"Zelda","url":"http://x/zelda.zip","size":"2.0M"}]"#);
        write(&root.join("games").join("SNES.json"), r#"{"games":[]}"#);
        write(&root.join("languages").join("en.json"), r#"{"loading":"Loading..."}"#);
        // 1x1 PNG (minimal geçerli imza baytları)
        write(&root.join("images").join("NES.png"),
            &[0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52]);
        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: None,
            show_unsupported: true,
            default_language: "en".into(),
            python: None,
        };
        (dir, cat)
    }

    #[tokio::test]
    async fn platforms_shape() {
        let (_d, cat) = fixture();
        let v = cat.get_json("/api/platforms").await.unwrap();
        assert_eq!(v["success"], true);
        assert_eq!(v["count"], 2);
        let plats = v["platforms"].as_array().unwrap();
        assert_eq!(plats[0]["platform_name"], "NES");
        assert_eq!(plats[0]["games_count"], 2);
        assert_eq!(plats[1]["games_count"], 0);
    }

    #[tokio::test]
    async fn search_shape() {
        let (_d, cat) = fixture();
        let v = cat.get_json("/api/search?q=zelda").await.unwrap();
        assert_eq!(v["success"], true);
        let games = v["results"]["games"].as_array().unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0]["game_name"], "Zelda");
        assert_eq!(games[0]["platform"], "NES");
        assert_eq!(games[0]["downloaded"], false);
    }

    #[tokio::test]
    async fn games_shape() {
        let (_d, cat) = fixture();
        let v = cat.get_json("/api/games/NES").await.unwrap();
        assert_eq!(v["platform"], "NES");
        assert_eq!(v["count"], 2);
        let games = v["games"].as_array().unwrap();
        assert_eq!(games[0]["name"], "Super Mario Bros");
        assert_eq!(games[0]["url"], "http://x/mario.zip");
    }

    #[tokio::test]
    async fn translations_shape() {
        let (_d, cat) = fixture();
        let v = cat.get_json("/api/translations").await.unwrap();
        assert_eq!(v["language"], "en");
        assert_eq!(v["translations"]["loading"], "Loading...");
        assert_eq!(v["translations"]["_language"], "en");
    }

    #[tokio::test]
    async fn image_shape() {
        let (_d, cat) = fixture();
        let (bytes, ct) = cat.get_image("NES").await.unwrap();
        assert!(ct.starts_with("image/png"));
        assert!(!bytes.is_empty());
    }
}

