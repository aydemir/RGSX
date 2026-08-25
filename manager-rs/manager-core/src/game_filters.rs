//! Saf oyun filtreleme mantığı — Python `game_filters.py` (`GameFilters`) 1:1 portu.
//!
//! TASK-002-gap-14 — `/api/save_filters` request contract'ı değişmez; bu modül
//! yalnızca saf iş mantığını (bölge çıkarımı, non-release tespiti, taban isim,
//! include/exclude + one-rom-per-game + bölge önceliği) Rust'a taşır ve
//! `contract.rs`/birim testleri ile parity'sini doğrular.
//!
//! BELİRSİZ kararlar çözüldü (2026-08-18):
//! - Modül = `manager-core` (saf mantık, I/O yok; `Settings.game_filters` zaten burada).
//! - TASK-006-native-settings-webui **zaten done** ve filtre mantığına dokunmaz → çakışma yok.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Mevcut bölgeler (Python `GameFilters.REGIONS`).
pub const REGIONS: &[&str] = &[
    "USA", "Canada", "Europe", "France", "Germany", "Japan", "Korea", "World", "Other",
];

/// Varsayılan bölge öncelik sırası (Python `GameFilters.region_priority`).
pub const DEFAULT_REGION_PRIORITY: &[&str] =
    &["USA", "Canada", "World", "Europe", "Japan", "Other"];

/// Filtrelenebilir oyun kaydı (Python `Game` tuple `(name, url, size)` karşılığı).
#[derive(Debug, Clone)]
pub struct FilteredGame {
    pub name: String,
    pub url: String,
    pub size: u64,
}

/// Oyun filtreleri (Python `GameFilters` parity'si).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GameFilters {
    /// Bölge -> "include" | "exclude".
    #[serde(default)]
    pub region_filters: HashMap<String, String>,
    #[serde(default)]
    pub hide_non_release: bool,
    #[serde(default)]
    pub one_rom_per_game: bool,
    #[serde(default)]
    pub hide_downloaded: bool,
    #[serde(default)]
    pub regex_mode: bool,
    #[serde(default)]
    pub region_priority: Vec<String>,
}

impl GameFilters {
    /// Tüm bölgeleri "include" + varsayılan seçeneklerle yeni filtre.
    pub fn new() -> Self {
        let region_filters = REGIONS
            .iter()
            .map(|r| (r.to_string(), "include".to_string()))
            .collect();
        Self {
            region_filters,
            hide_non_release: false,
            one_rom_per_game: false,
            hide_downloaded: false,
            regex_mode: false,
            region_priority: DEFAULT_REGION_PRIORITY
                .iter()
                .map(|s| s.to_string())
                .collect(),
        }
    }

    /// `Settings.extra["game_filters"]` (veya benzeri) dict'ten yükler; Python
    /// `load_from_dict` parity'si — tüm bölgeler önce "include", sonra yüklenenler uygulanır.
    pub fn load_from_dict(&mut self, d: &serde_json::Value) {
        let mut rf: HashMap<String, String> = REGIONS
            .iter()
            .map(|r| (r.to_string(), "include".to_string()))
            .collect();
        if let Some(obj) = d.get("region_filters").and_then(|v| v.as_object()) {
            for region in REGIONS {
                let state = obj
                    .get(*region)
                    .and_then(|v| v.as_str())
                    .unwrap_or("include");
                rf.insert(region.to_string(), state.to_string());
            }
        }
        self.region_filters = rf;
        self.hide_non_release = d
            .get("hide_non_release")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.one_rom_per_game = d
            .get("one_rom_per_game")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.hide_downloaded = d
            .get("hide_downloaded")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        self.regex_mode = d
            .get("regex_mode")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if let Some(arr) = d.get("region_priority").and_then(|v| v.as_array()) {
            let v: Vec<String> = arr
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect();
            if !v.is_empty() {
                self.region_priority = v;
            }
        }
    }

    /// Kaydetmek için dict'e çevirir (Python `to_dict` parity'si).
    pub fn to_dict(&self) -> serde_json::Value {
        serde_json::json!({
            "region_filters": self.region_filters,
            "hide_non_release": self.hide_non_release,
            "one_rom_per_game": self.one_rom_per_game,
            "hide_downloaded": self.hide_downloaded,
            "regex_mode": self.regex_mode,
            "region_priority": self.region_priority,
        })
    }

    /// En az bir filtre aktif mi (Python `is_active` parity'si).
    pub fn is_active(&self) -> bool {
        let has_exclude = self.region_filters.values().any(|s| s == "exclude");
        has_exclude || self.hide_non_release || self.one_rom_per_game || self.hide_downloaded
    }

    /// Tüm filtreleri sıfırlar (Python `reset` parity'si).
    pub fn reset(&mut self) {
        self.region_filters = REGIONS
            .iter()
            .map(|r| (r.to_string(), "include".to_string()))
            .collect();
        self.hide_non_release = false;
        self.one_rom_per_game = false;
        self.hide_downloaded = false;
        self.regex_mode = false;
    }

    /// Bölge öncelik skoru: `region_priority` içindeki en düşük indeks (düşük = iyi),
    /// listede yoksa uzunluk (en düşük öncelik). Python `get_region_priority` parity'si.
    fn region_priority_score(&self, regions: &[String]) -> usize {
        let mut best = self.region_priority.len();
        for r in regions {
            if let Some(i) = self.region_priority.iter().position(|x| x == r) {
                if i < best {
                    best = i;
                }
            }
        }
        best
    }

    /// Filtreleri bir oyun listesine uygular (Python `apply_filters` parity'si).
    ///
    /// `is_downloaded` yalnız `hide_downloaded` aktifken kullanılır (pure-test için
    /// enjekte edilir; Python'daki `is_game_downloaded` karşılığı). `is_active()`
    /// false ise liste değişmeden döner.
    pub fn apply_filters(
        &self,
        games: &[FilteredGame],
        is_downloaded: impl Fn(&str) -> bool,
    ) -> Vec<FilteredGame> {
        if !self.is_active() {
            return games.to_vec();
        }
        let has_excl = self.region_filters.values().any(|s| s == "exclude");
        let mut out: Vec<FilteredGame> = Vec::new();
        for g in games {
            if has_excl {
                let gr = get_game_regions(&g.name);
                let included = gr.iter().any(|r| {
                    self.region_filters
                        .get(r)
                        .map(|s| s == "include")
                        .unwrap_or(true)
                });
                if !included {
                    continue;
                }
            }
            if self.hide_non_release && is_non_release_game(&g.name) {
                continue;
            }
            if self.hide_downloaded && is_downloaded(&g.name) {
                continue;
            }
            out.push(g.clone());
        }

        if self.one_rom_per_game {
            let mut by_base: HashMap<String, Vec<FilteredGame>> = HashMap::new();
            for g in out {
                let base = get_base_game_name(&g.name);
                by_base.entry(base).or_default().push(g);
            }
            let mut res: Vec<FilteredGame> = Vec::new();
            for (_base, list) in by_base {
                if list.len() == 1 {
                    res.push(list.into_iter().next().unwrap());
                } else {
                    let mut sorted = list;
                    sorted.sort_by_key(|g| {
                        let gr = get_game_regions(&g.name);
                        self.region_priority_score(&gr)
                    });
                    res.push(sorted.into_iter().next().unwrap());
                }
            }
            out = res;
        }
        out
    }
}

// --- Saf yardımcılar (Python `@staticmethod` karşılıkları) ---

fn lazy_paren() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\(([^)]+)\)").unwrap())
}

fn lazy_disc() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)(?:\(|\[)?(?:Dis[ck]|CD)\s*(\d+)(?:\)|\])?").unwrap())
}

fn lazy_non_release() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(
            r"\([^)]*(BETA|DEMO|PROTO|SAMPLE|KIOSK|PREVIEW|TEST|DEBUG|ALPHA|PRE-RELEASE|PRERELEASE|UNFINISHED|WIP|BOOTLEG)[^)]*\)",
        )
        .unwrap()
    })
}

fn lazy_ext() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\.(zip|7z|rar|gz|iso)$").unwrap())
}

fn lazy_parens_strip() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\([^)]*\)").unwrap())
}

fn lazy_brackets_strip() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[[^\]]*\]").unwrap())
}

/// ASCII kelime sınırı kontrolü (`\bWORD\b` için, regex'siz ve verimli).
fn contains_word(hay: &str, word: &str) -> bool {
    let h = hay.as_bytes();
    let w = word.as_bytes();
    let (n, m) = (h.len(), w.len());
    if m == 0 || m > n {
        return false;
    }
    let mut i = 0;
    while i + m <= n {
        if &h[i..i + m] == w {
            let prev = i == 0 || !h[i - 1].is_ascii_alphanumeric();
            let next = i + m == n || !h[i + m].is_ascii_alphanumeric();
            if prev && next {
                return true;
            }
        }
        i += 1;
    }
    false
}

/// İsimden bölgeleri çıkarır (Python `get_game_regions` parity'si).
pub fn get_game_regions(name: &str) -> Vec<String> {
    let name_up = name.to_uppercase();
    let mut regions: Vec<String> = Vec::new();
    let push_unique = |regions: &mut Vec<String>, r: &str| {
        if !regions.iter().any(|x| x == r) {
            regions.push(r.to_string());
        }
    };

    // Parantez içi kodlar (Fr,De / En,Nl vb.)
    for cap in lazy_paren().captures_iter(&name_up) {
        let content = &cap[1];
        let codes: Vec<&str> = content.split(',').map(|c| c.trim()).collect();
        for code in &codes {
            if code.is_empty() {
                continue;
            }
            match *code {
                "FR" | "FRA" => push_unique(&mut regions, "France"),
                "DE" | "GER" | "DEU" => push_unique(&mut regions, "Germany"),
                "EN" | "ENG" | _ if code.starts_with("EN-") => {
                    // EN belirsiz; yalnız EU/EUR da varsa Europe
                    if codes.iter().any(|c| *c == "EU" || *c == "EUR") {
                        push_unique(&mut regions, "Europe");
                    }
                }
                "ES" | "ESP" | "SPA" => push_unique(&mut regions, "Other"),
                "IT" | "ITA" => push_unique(&mut regions, "Other"),
                "NL" | "NLD" | "DU" | "DUT" => push_unique(&mut regions, "Europe"),
                "PT" | "POR" => push_unique(&mut regions, "Other"),
                _ => {}
            }
        }
    }

    // Tam kelime bölge kontrolleri
    if name_up.contains("USA") || name_up.contains("US)") || contains_word(&name_up, "US") {
        push_unique(&mut regions, "USA");
    }
    if name_up.contains("CANADA") || name_up.contains("CA)") {
        push_unique(&mut regions, "Canada");
    }
    if name_up.contains("EUROPE") || name_up.contains("EU)") || contains_word(&name_up, "EU") {
        push_unique(&mut regions, "Europe");
    }
    if name_up.contains("FRANCE") || name_up.contains("FR)") {
        push_unique(&mut regions, "France");
    }
    if name_up.contains("GERMANY") || name_up.contains("DE)") || name_up.contains("GER)") {
        push_unique(&mut regions, "Germany");
    }
    if name_up.contains("JAPAN")
        || name_up.contains("JP)")
        || name_up.contains("JPN)")
        || contains_word(&name_up, "JP")
    {
        push_unique(&mut regions, "Japan");
    }
    if name_up.contains("KOREA") || name_up.contains("KR)") || name_up.contains("KOR)") {
        push_unique(&mut regions, "Korea");
    }
    if name_up.contains("WORLD") {
        push_unique(&mut regions, "World");
    }

    // Diğer bölgeler -> Other
    for w in [
        "AUSTRALIA",
        "ASIA",
        "BRAZIL",
        "CHINA",
        "RUSSIA",
        "SCANDINAVIA",
        "SPAIN",
        "ITALY",
    ] {
        if contains_word(&name_up, w) {
            push_unique(&mut regions, "Other");
            break;
        }
    }

    if regions.is_empty() {
        regions.push("Other".to_string());
    }
    regions
}

/// Non-release sürüm mü? (beta/demo/proto/sample/wip/bootleg vb.) — Python
/// `is_non_release_game` parity'si.
pub fn is_non_release_game(name: &str) -> bool {
    lazy_non_release().is_match(&name.to_uppercase())
}

/// Taban oyun ismini döndürür (bölge/versiyon/uzantı soyulmuş; one-rom-per-game
/// için). Python `get_base_game_name` parity'si.
pub fn get_base_game_name(name: &str) -> String {
    let mut base = lazy_ext().replace_all(name, "").to_string();

    // Disk bilgisini koru: "(Disc 1)" / "[Disc 2]" / "Disc 3" / "(CD 1)"
    let disc_info = lazy_disc()
        .captures(&base)
        .map(|c| format!(" (Disc {})", &c[1]))
        .unwrap_or_default();

    base = lazy_parens_strip().replace_all(&base, "").to_string();
    base = lazy_brackets_strip().replace_all(&base, "").to_string();
    base = base.split_whitespace().collect::<Vec<_>>().join(" ");

    format!("{base}{disc_info}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn g(name: &str) -> FilteredGame {
        FilteredGame {
            name: name.to_string(),
            url: format!("http://x/{}", name),
            size: 0,
        }
    }

    #[test]
    fn regions_from_name() {
        assert_eq!(
            get_game_regions("Super Mario (USA)"),
            vec!["USA".to_string()]
        );
        assert_eq!(
            get_game_regions("Mega Man (EU)"),
            vec!["Europe".to_string()]
        );
        assert_eq!(
            get_game_regions("Game (Fr,De)"),
            vec!["France".to_string(), "Germany".to_string()]
        );
        // EN yalnız başına belirsiz -> France (EU/EUR yok)
        assert_eq!(
            get_game_regions("Zelda (En,Fr)"),
            vec!["France".to_string()]
        );
        // EN + EU -> Europe
        assert_eq!(get_game_regions("Game (En,Eu)"), vec!["Europe".to_string()]);
        assert_eq!(get_game_regions("Sonic (World)"), vec!["World".to_string()]);
        assert_eq!(get_game_regions("JRPG (Japan)"), vec!["Japan".to_string()]);
        // bölge yok -> Other
        assert_eq!(get_game_regions("Mystery Game"), vec!["Other".to_string()]);
    }

    #[test]
    fn non_release_detection() {
        assert!(is_non_release_game("Game (Beta)"));
        assert!(is_non_release_game("Game (DEMO)"));
        assert!(is_non_release_game("Game (Proto)"));
        assert!(is_non_release_game("Game (Sample)"));
        assert!(is_non_release_game("Game (WIP)"));
        assert!(is_non_release_game("Game (Bootleg)"));
        assert!(is_non_release_game("Game (Pre-Release)"));
        assert!(!is_non_release_game("Super Mario (USA)"));
        assert!(!is_non_release_game("Normal Game"));
    }

    #[test]
    fn base_game_name_strips_regions_and_ext() {
        assert_eq!(get_base_game_name("Super Mario (USA).zip"), "Super Mario");
        assert_eq!(get_base_game_name("Castlevania (Beta)(USA)"), "Castlevania");
        assert_eq!(get_base_game_name("Game (Disc 1)"), "Game (Disc 1)");
        assert_eq!(get_base_game_name("Game [Disc 2]"), "Game (Disc 2)");
        assert_eq!(get_base_game_name("Tetris   (World)"), "Tetris");
    }

    #[test]
    fn apply_filters_region_exclude() {
        let mut f = GameFilters::new();
        f.region_filters
            .insert("Europe".to_string(), "exclude".to_string());
        let games = vec![g("A (USA)"), g("B (Europe)"), g("C (World)")];
        let out = f.apply_filters(&games, |_| false);
        let names: Vec<&str> = out.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["A (USA)", "C (World)"]);
    }

    #[test]
    fn apply_filters_hide_non_release() {
        let mut f = GameFilters::new();
        f.hide_non_release = true;
        let games = vec![g("Real Game (USA)"), g("Leak (Beta)")];
        let out = f.apply_filters(&games, |_| false);
        let names: Vec<&str> = out.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, vec!["Real Game (USA)"]);
    }

    #[test]
    fn apply_filters_one_rom_per_game_keeps_highest_priority_region() {
        let mut f = GameFilters::new();
        f.one_rom_per_game = true;
        // Aynı taban isim, iki ROM: biri USA (öncelik 0), biri Europe (öncelik 3)
        let games = vec![g("Cool Game (Europe)"), g("Cool Game (USA)")];
        let out = f.apply_filters(&games, |_| false);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].name, "Cool Game (USA)");
    }

    #[test]
    fn apply_filters_inactive_returns_all() {
        let f = GameFilters::new(); // hiçbiri aktif değil
        let games = vec![g("A (USA)"), g("B (Beta)")];
        let out = f.apply_filters(&games, |_| false);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn load_and_to_dict_roundtrip() {
        let mut f = GameFilters::new();
        f.region_filters
            .insert("Europe".to_string(), "exclude".to_string());
        f.hide_non_release = true;
        f.one_rom_per_game = true;
        let v = f.to_dict();
        let mut f2 = GameFilters::new();
        f2.load_from_dict(&v);
        assert_eq!(f2.region_filters.get("Europe").unwrap(), "exclude");
        assert_eq!(f2.region_filters.get("USA").unwrap(), "include");
        assert!(f2.hide_non_release);
        assert!(f2.one_rom_per_game);
    }

    #[test]
    fn load_from_dict_defaults_missing_regions_to_include() {
        let v = serde_json::json!({ "region_filters": { "Japan": "exclude" } });
        let mut f = GameFilters::new();
        f.load_from_dict(&v);
        assert_eq!(f.region_filters.get("Japan").unwrap(), "exclude");
        // yüklenmeyen bölgeler include kalır
        assert_eq!(f.region_filters.get("USA").unwrap(), "include");
        assert_eq!(f.region_filters.get("Other").unwrap(), "include");
    }
}
