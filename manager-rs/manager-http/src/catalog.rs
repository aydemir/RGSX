//! Katalog route'ları için `CatalogSource` trait + `NativeCatalog`.
//!
//! Tarihçe (arşiv): Faz 10c'de bu route'lar önce `RGSX_PYTHON_MANAGER_URL`
//! üzerinden Python manager proxy'siyle (`PythonCatalog`) çalıştı, Faz 12c ile
//! yerini Python'sız local-dosya `NativeCatalog`'a bıraktı. Python kalıntısı
//! TASK-012-gap-02 ile tamamen söküldü — tek kaynak `NativeCatalog`.

use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use manager_core::settings::Settings;
use serde_json::Value;

/// Katalog kaynağı hatası (proxy çökmesi → handler placeholder'a düşer).
#[derive(Debug)]
pub struct CatalogError(pub String);

/// Oyun adını karşılaştırma için normalize eder (küçük harf + alfanümerik only).
/// Python `normalize_game_name` eşleniği.
pub fn norm_game_name(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Vue `stem(name)` ile birebir aynı: küçük harf + son soneki soy.
/// `gameStatusOf` bu anahtarla bakar; `game_statuses` haritası da bu formda
/// (ve tam küçük harfli adla) anahtarlanmalı ki ön yüz yeşil rozeti görsün.
fn vue_stem(s: &str) -> String {
    let lower = s.to_lowercase();
    match lower.rfind('.') {
        Some(i) if i > 0 => lower[..i].to_string(),
        _ => lower,
    }
}

/// Verilen dizindeki (özyinelemeli) dosyaların normalize stem'lerini toplar.
/// Metadata/görsel uzantılarını atlar (oyun dosyası değil).
fn collect_disk_stems(dir: &Path, out: &mut HashSet<String>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect_disk_stems(&p, out);
        } else if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if matches!(
                ext.as_str(),
                "json"
                    | "txt"
                    | "md"
                    | "db"
                    | "log"
                    | "png"
                    | "jpg"
                    | "jpeg"
                    | "gif"
                    | "svg"
                    | "webp"
            ) {
                continue;
            }
            out.insert(norm_game_name(stem));
        }
    }
}

/// Katalog veri kaynağı — test'te `FakeCatalog` ile enjekte edilebilir.
#[async_trait]
pub trait CatalogSource: Send + Sync {
    /// JSON dönen GET route'u proxy'ler (ör. `/api/platforms`, `/api/search?q=zelda`).
    async fn get_json(&self, route: &str) -> Result<Value, CatalogError>;
    /// JSON dönen POST route'u proxy'ler (gövde iletilir).
    async fn post_json(&self, route: &str, body: &Value) -> Result<Value, CatalogError>;
    /// İkili (zip) POST route'u proxy'ler (ham bayt + content-type).
    async fn post_binary(
        &self,
        route: &str,
        body: &Value,
    ) -> Result<(Vec<u8>, String), CatalogError>;
    /// Box-art görselini (ham bayt + content-type) proxy'ler.
    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError>;

    /// Faz 12.6a — diskte kurulu/indirilmiş oyunların taramayla bulunmuş listesi.
    /// Dönüş: `platform_name ->` o platformda diskte bulunan oyun adları.
    /// Varsayılan boş döner; `NativeCatalog` gerçek taramayı yapar.
    fn installed_list(&self) -> HashMap<String, Vec<String>> {
        HashMap::new()
    }

    /// Faz 12.6a — `/api/game-status` yanıtı: `stem -> {status, platform, name}`.
    /// Varsayılan boş `statuses` döner.
    fn game_statuses(&self) -> Value {
        serde_json::json!({ "statuses": {} })
    }

    /// Faz 12.6d — batch indirme için `platform + game_name` → oyun URL'i çözümü.
    /// `NativeCatalog` katalogdan bulur; varsayılan `None` döner.
    fn game_url(&self, _platform: &str, _game_name: &str) -> Option<String> {
        None
    }
}

// ===========================================================================
// Faz 12c — NativeCatalog: local dosyalardan catalog üretimi.
//
// Veri kaynağı Python `get_cached_sources()`/`load_sources()`/`load_games()`
// ile birebir aynı dosyalardır (systems_list.json, games/<platform>.json,
// languages/<lang>.json, images/<platform>.*); böylece çıktı şekli Python
// referansıyla aynı kalır ve offline çalışır. Komut POST'ları manager-http
// handler'larında native işlenir (TASK-013 sonrası tek yol).
// ===========================================================================

use std::path::{Path, PathBuf};

/// Box-art uzantısı → content-type.
fn image_content_type(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
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
        Self {
            sources_file,
            games_folder,
            images_folder,
            languages_folder,
            roms_folder,
            show_unsupported,
            default_language,
        }
    }

    /// Faz 12.6e — efektif ROM kökü: kullanıcı `settings.roms_folder` ayarladıysa
    /// onu kullan, yoksa env `RGSX_ROMS_FOLDER` (self.roms_folder) fallback.
    fn effective_roms_folder(&self) -> Option<PathBuf> {
        let s = Settings::load();
        let rf = s.roms_folder.trim();
        if !rf.is_empty() {
            return Some(PathBuf::from(rf));
        }
        self.roms_folder.clone()
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
                    let legacy = m
                        .remove("system_image")
                        .and_then(|v| v.as_str().map(str::to_string))
                        .unwrap_or_default();
                    m.insert("platform_image".into(), Value::String(legacy));
                }
                if !m.contains_key("folder") {
                    if let Some(f) = m
                        .get("dossier")
                        .and_then(|v| v.as_str())
                        .map(str::to_string)
                    {
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
                let name = s
                    .get("platform_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let folder = s.get("folder").and_then(|v| v.as_str()).unwrap_or("");
                let ok_name =
                    !name.is_empty() && existing_files.contains(&name.to_ascii_lowercase());
                let ok_folder =
                    !folder.is_empty() && existing_files.contains(&folder.to_ascii_lowercase());
                name.is_empty() || ok_name || ok_folder
            })
            .collect()
    }

    /// `games/<platform>.json` → [{name,url,size}] (Python `load_games` sadeleştirilmiş).
    /// Bir platformın oyun dosyasını çözer: önce `games/<platform_name>.json`,
    /// yoksa `games/<folder>.json` (hem orijinal ad hem küçük harf). OTA katalog
    /// verisi bazen klasör adını, bazen platform adını kullanır; her ikisini de
    /// kabul ederiz ki `platform_name != folder` olan platformlar (ör. "Game Boy")
    /// drop olmasın (Faz 12.1 — platform yükleme eksik veri sorunu).
    fn games_file_for(&self, platform: &str) -> Option<PathBuf> {
        let mut candidates: Vec<String> = vec![platform.to_string(), platform.to_ascii_lowercase()];
        if let Some(src) = self.load_sources().iter().find(|s| {
            s.get("platform_name").and_then(|x| x.as_str()) == Some(platform)
                || s.get("folder").and_then(|x| x.as_str()) == Some(platform)
        }) {
            // Python catalog games dosyaları platform ADIYLA adlandırılır
            // (ör. "3DO Interactive Multiplayer (Archive).json"), Rust sorgusu ise
            // folder ile gelir ("3do"). Her iki adı da aday olarak ekle ki dosya bulunabilsin.
            if let Some(name) = src.get("platform_name").and_then(|x| x.as_str()) {
                candidates.push(name.to_string());
                candidates.push(name.to_ascii_lowercase());
            }
            if let Some(folder) = src.get("folder").and_then(|x| x.as_str()) {
                candidates.push(folder.to_string());
                candidates.push(folder.to_ascii_lowercase());
            }
        }
        candidates.iter().find_map(|c| {
            let p = self.games_folder.join(format!("{c}.json"));
            p.is_file().then_some(p)
        })
    }

    fn load_games(&self, platform: &str) -> Vec<(String, Option<String>, Option<String>)> {
        let file = match self.games_file_for(platform) {
            Some(f) => f,
            None => return vec![],
        };
        let data: Value = match std::fs::read_to_string(&file) {
            Ok(txt) => serde_json::from_str(&txt).unwrap_or(Value::Null),
            Err(_) => return vec![],
        };
        let arr = match &data {
            Value::Array(a) => a.clone(),
            Value::Object(m) if m.contains_key("games") => m
                .get("games")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
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
                    let url = t
                        .get(1)
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(str::to_string);
                    let size = t
                        .get(2)
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(str::to_string);
                    if !name.is_empty() {
                        out.push((name, url, size));
                    }
                }
                Value::Object(m) => {
                    let name = m
                        .get("game_name")
                        .or_else(|| m.get("name"))
                        .or_else(|| m.get("title"))
                        .or_else(|| m.get("game"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if name.is_empty() {
                        continue;
                    }
                    let url = m
                        .get("url")
                        .or_else(|| m.get("download"))
                        .or_else(|| m.get("link"))
                        .or_else(|| m.get("href"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(str::to_string);
                    let size = m
                        .get("size")
                        .or_else(|| m.get("filesize"))
                        .or_else(|| m.get("length"))
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.trim().is_empty())
                        .map(str::to_string);
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
            let name = s
                .get("platform_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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
            let name = s
                .get("platform_name")
                .and_then(|v| v.as_str())
                .unwrap_or("");
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

    fn build_translations(&self, lang: &str) -> Value {
        let lang = if lang.is_empty() {
            &self.default_language
        } else {
            lang
        };
        let file = self.languages_folder.join(format!("{lang}.json"));
        let translations = match std::fs::read_to_string(&file) {
            Ok(txt) => {
                serde_json::from_str::<Value>(&txt).unwrap_or(Value::Object(Default::default()))
            }
            Err(_) => Value::Object(Default::default()),
        };
        let mut t = match translations {
            Value::Object(m) => m,
            _ => serde_json::Map::new(),
        };
        t.insert("_language".into(), Value::String(lang.to_string()));
        serde_json::json!({ "success": true, "language": lang, "translations": Value::Object(t) })
    }

    /// `languages_folder` içindeki `*.json` dosyalarından dil kodlarını listeler (TASK-003).
    fn list_languages(&self) -> Value {
        let mut langs: Vec<String> = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.languages_folder) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) == Some("json") {
                    if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                        langs.push(stem.to_string());
                    }
                }
            }
        }
        langs.sort();
        serde_json::json!({ "success": true, "languages": langs })
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
            let q = route
                .split_once('?')
                .map(|(_, q)| parse_query(q))
                .and_then(|m| m.get("q").cloned())
                .unwrap_or_default();
            return Ok(self.build_search(&q));
        }
        if route.starts_with("/api/games/") {
            let platform = pct_decode(route.trim_start_matches("/api/games/"));
            return Ok(self.build_games(&platform));
        }
        if route.starts_with("/api/translations") {
            let lang = route
                .split_once('?')
                .map(|(_, q)| parse_query(q))
                .and_then(|m| m.get("lang").cloned())
                .unwrap_or_default();
            return Ok(self.build_translations(&lang));
        }
        if route.starts_with("/api/languages") {
            return Ok(self.list_languages());
        }
        Err(CatalogError(format!(
            "native catalog desteklemiyor: {route}"
        )))
    }

    async fn post_json(&self, route: &str, _body: &Value) -> Result<Value, CatalogError> {
        Err(CatalogError(format!(
            "native catalog POST desteklemiyor: {route}"
        )))
    }

    async fn post_binary(
        &self,
        route: &str,
        _body: &Value,
    ) -> Result<(Vec<u8>, String), CatalogError> {
        Err(CatalogError(format!(
            "native catalog POST desteklemiyor: {route}"
        )))
    }

    async fn get_image(&self, platform: &str) -> Result<(Vec<u8>, String), CatalogError> {
        if let Some(img) = self.read_image(platform) {
            return Ok(img);
        }
        Err(CatalogError(format!("image bulunamadı: {platform}")))
    }

    /// Faz 12.6a — diskte kurulu oyunların taraması.
    ///
    /// `RGSX_ROMS_FOLDER` (ya da data_dir altındaki olası ROM kökleri) içinde her
    /// platformun `folder` (veya `platform_name`) dizinini özyinelemeli tarar;
    /// katalogdaki oyun adıyla normalize stem eşleşen dosyaları "indirilmiş" sayar.
    /// Python `history.scan_roms_for_downloaded_games` / `game_list.is_game_downloaded`
    /// eşleniği.
    fn installed_list(&self) -> HashMap<String, Vec<String>> {
        let mut out: HashMap<String, Vec<String>> = HashMap::new();

        let data_dir = self
            .sources_file
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let mut roots: Vec<PathBuf> = Vec::new();
        if let Some(r) = self.effective_roms_folder() {
            roots.push(r);
        }
        for cand in [
            data_dir.join("downloads"),
            data_dir.join("roms").join("ports").join("RGSX"),
            data_dir.join("roms"),
            data_dir.join("ports").join("RGSX"),
        ] {
            if cand.is_dir() {
                roots.push(cand);
            }
        }
        if roots.is_empty() {
            return out;
        }

        for s in self.load_sources() {
            let name = s
                .get("platform_name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if name.is_empty() {
                continue;
            }
            let folder = s
                .get("folder")
                .and_then(|v| v.as_str())
                .unwrap_or(&name)
                .to_string();

            let mut disk_stems: HashSet<String> = HashSet::new();
            for root in &roots {
                collect_disk_stems(&root.join(&folder), &mut disk_stems);
                collect_disk_stems(&root.join(&name), &mut disk_stems);
            }
            if disk_stems.is_empty() {
                continue;
            }

            let mut found: Vec<String> = Vec::new();
            for (gname, _, _) in self.load_games(&name) {
                // Katalog oyun adındaki soneki (".chd", ".zip"...) disk stem'i ile
                // aynı şekilde soyup normalize et (Python `is_game_downloaded`).
                let gstem = std::path::Path::new(&gname)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or(gname.as_str())
                    .to_string();
                if disk_stems.contains(&norm_game_name(&gstem)) {
                    found.push(gname);
                }
            }
            if !found.is_empty() {
                out.insert(name, found);
            }
        }
        out
    }

    /// Vue `stem(name)` ile birebir aynı: küçük harf + son soneki soy.
    /// Faz 12.6a — `/api/game-status` yanıtı. `installed_list` sonucunu
    /// `stem -> {status:"downloaded", platform, name}` eşlemesine dönüştürür.
    fn game_statuses(&self) -> Value {
        let installed = self.installed_list();
        let mut statuses: HashMap<String, Value> = HashMap::new();
        for (platform, names) in &installed {
            for name in names {
                let val = serde_json::json!({
                    "status": "downloaded",
                    "platform": platform,
                    "name": name,
                });
                // Vue `gameStatusOf` `s[stem(g.name)]` ve `s[g.name.toLowerCase()]`
                // bakar; her ikisini de anahtarla.
                statuses.insert(vue_stem(name), val.clone());
                statuses.insert(name.to_lowercase(), val);
            }
        }
        serde_json::json!({ "statuses": statuses })
    }

    /// Faz 12.6d — `platform + game_name` → katalogdaki oyun URL'i.
    fn game_url(&self, platform: &str, game_name: &str) -> Option<String> {
        self.load_games(platform)
            .into_iter()
            .find(|(n, u, _)| n == game_name && u.is_some())
            .and_then(|(_, u, _)| u)
    }
}

fn env_path(key: &str, default: PathBuf) -> PathBuf {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or(default)
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
        write(
            &root.join("systems_list.json"),
            r#"[{"platform_name":"NES","folder":"nes","platform_image":"nes.png"},{"platform_name":"SNES","folder":"snes"}]"#,
        );
        write(
            &root.join("games").join("NES.json"),
            r#"[["Super Mario Bros","http://x/mario.zip","1.2M"],{"game_name":"Zelda","url":"http://x/zelda.zip","size":"2.0M"}]"#,
        );
        write(&root.join("games").join("SNES.json"), r#"{"games":[]}"#);
        write(
            &root.join("languages").join("en.json"),
            r#"{"loading":"Loading..."}"#,
        );
        // 1x1 PNG (minimal geçerli imza baytları)
        write(
            &root.join("images").join("NES.png"),
            &[
                0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49,
                0x48, 0x44, 0x52,
            ],
        );
        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: None,
            show_unsupported: true,
            default_language: "en".into(),
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

    // --- TASK-011: her iki games JSON formatı da desteklenmeli ---

    #[tokio::test]
    async fn games_list_format() {
        // Format B: [[name, url, size], ...]
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(
            &root.join("systems_list.json"),
            r#"[{"platform_name":"XBOX","folder":"xbox"}]"#,
        );
        write(
            &root.join("games").join("XBOX.json"),
            r#"[["Halo","http://x/halo.zip","3.0G"],["Forza","http://x/forza.zip","40.0G"]]"#,
        );
        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: None,
            show_unsupported: true,
            default_language: "en".into(),
        };
        let v = cat.get_json("/api/games/xbox").await.unwrap();
        assert_eq!(v["count"], 2);
        let g = v["games"].as_array().unwrap();
        assert_eq!(g[0]["name"], "Halo");
        assert_eq!(g[0]["url"], "http://x/halo.zip");
        assert_eq!(g[0]["size"], "3.0G");
        assert_eq!(g[0]["downloaded"], false);
    }

    #[tokio::test]
    async fn games_object_format() {
        // Format A: {"games":[{"game_name":...,"url":...,"size":...}]}
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(
            &root.join("systems_list.json"),
            r#"[{"platform_name":"XBOX","folder":"xbox"}]"#,
        );
        write(
            &root.join("games").join("XBOX.json"),
            r#"{"games":[{"game_name":"Halo","url":"http://x/halo.zip","size":"3.0G"},{"name":"Forza","url":"http://x/forza.zip","size":"40.0G"}]}"#,
        );
        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: None,
            show_unsupported: true,
            default_language: "en".into(),
        };
        let v = cat.get_json("/api/games/xbox").await.unwrap();
        assert_eq!(v["count"], 2);
        let g = v["games"].as_array().unwrap();
        assert_eq!(g[0]["name"], "Halo");
        assert_eq!(g[1]["name"], "Forza");
        assert_eq!(g[0]["size"], "3.0G");
    }

    #[tokio::test]
    async fn games_resolve_by_platform_name_file() {
        // Gerçek senaryo: games dosyası platform ADIYLA adlandırılmış, sorgu folder ile gelir.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(
            &root.join("systems_list.json"),
            r#"[{"platform_name":"3DO Interactive Multiplayer (Archive)","folder":"3do"}]"#,
        );
        write(
            &root
                .join("games")
                .join("3DO Interactive Multiplayer (Archive).json"),
            r#"[["Almanac","http://x/a.chd","382.0M"]]"#,
        );
        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: None,
            show_unsupported: true,
            default_language: "en".into(),
        };
        // WebUI folder ("3do") ile sorgular:
        let v = cat.get_json("/api/games/3do").await.unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["games"][0]["name"], "Almanac");
    }

    #[tokio::test]
    async fn platforms_resolve_by_folder() {
        // Faz 12.1: `platform_name != folder` olan bir platformun oyun dosyası
        // klasör adıyla (`gb.json`) isimlenmişse bile yüklenmeli (drop olmamalı).
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(
            &root.join("systems_list.json"),
            r#"[{"platform_name":"Game Boy","folder":"gb","platform_image":"gb.png"},{"platform_name":"NES","folder":"nes"}]"#,
        );
        // Oyun dosyası platform ADIYLA değil KLASÖR adıyla isimli:
        write(
            &root.join("games").join("gb.json"),
            r#"[{"game_name":"Tetris","url":"http://x/tetris.zip","size":"0.5M"}]"#,
        );
        write(&root.join("games").join("nes.json"), r#"{"games":[]}"#);
        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: None,
            show_unsupported: true,
            default_language: "en".into(),
        };
        let v = cat.get_json("/api/platforms").await.unwrap();
        assert_eq!(
            v["count"], 2,
            "tüm platformlar (name!=folder dahil) yüklenmeli"
        );
        let plats = v["platforms"].as_array().unwrap();
        let gb = plats
            .iter()
            .find(|p| p["platform_name"] == "Game Boy")
            .expect("Game Boy drop olmamalı");
        assert_eq!(
            gb["games_count"], 1,
            "gb.json (folder ile isimli) çözülmeli"
        );
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

    #[test]
    fn installed_list_detects_disk_roms() {
        // Faz 12.6a: roms kökünde (RGSX_ROMS_FOLDER) bir platform klasörüne
        // koyulan ROM dosyası, katalogdaki oyunla eşleşmeli ve `installed_list`
        // içinde `downloaded` olarak görünmeli.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(
            &root.join("systems_list.json"),
            r#"[{"platform_name":"NES","folder":"nes"},{"platform_name":"SNES","folder":"snes"}]"#,
        );
        write(
            &root.join("games").join("NES.json"),
            r#"[["Super Mario Bros","http://x/mario.zip","1.2M"],["Zelda","http://x/zelda.zip","2.0M"]]"#,
        );
        write(&root.join("games").join("snes.json"), r#"{"games":[]}"#);

        // Diskte kurulu bir ROM: normalize("Super Mario Bros") == "supermariobros".
        write(
            &root.join("roms").join("nes").join("Super Mario Bros.nes"),
            b"ROM",
        );

        let cat = NativeCatalog {
            sources_file: root.join("systems_list.json"),
            games_folder: root.join("games"),
            images_folder: root.join("images"),
            languages_folder: root.join("languages"),
            roms_folder: Some(root.join("roms")),
            show_unsupported: true,
            default_language: "en".into(),
        };

        let installed = cat.installed_list();
        assert_eq!(
            installed.get("NES").map(|v| v.as_slice()),
            Some(&["Super Mario Bros".to_string()][..]),
            "diskteki ROM kurulu oyun olarak bulunmalı"
        );
        assert!(
            installed.get("SNES").is_none(),
            "ROM'suz platform boş olmalı"
        );

        let statuses = cat.game_statuses();
        let st = statuses["statuses"].as_object().unwrap();
        // Vue `stem("Super Mario Bros")` -> "super mario bros" (son ek soyulur).
        let entry = &st["super mario bros"];
        assert_eq!(entry["status"], "downloaded");
        assert_eq!(entry["platform"], "NES");
        assert_eq!(entry["name"], "Super Mario Bros");
    }
}
