//! TASK-012h Faz 1 — SDL'siz çekirdek state machine + SAF input reducer.
//!
//! `tvui.py` `config.menu_state` dispatch'inin tip-güvenli Rust karşılığı.
//! SDL yalnız piksel işi yapar; karar/test edilebilir her şey burada, SDL'siz.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

use crate::accessibility::Accessibility;
use crate::folder_browser::{BrowserMode, FolderBrowser};
use crate::menus::{MenuKind, MenuNav};
use crate::net::{PlatformTile, TvuiState, UiAction, UiKey};
use crate::render::Transition;
use crate::virtual_keyboard::{KeyboardVariant, VirtualKeyboard};

/// Izgara geometrisi — Python `config.GRID_COLS=3 / GRID_ROWS=4` parity
/// (`config.py:451`). Hem `draw_grid` hem bu modüldeki nav aynı sabitleri kullanır.
pub const GRID_COLS: usize = 3;
pub const GRID_ROWS: usize = 4;
/// Sayfa başına platform (`GRID_COLS × GRID_ROWS`, Python `systems_per_page`).
pub const GRID_PER_PAGE: usize = GRID_COLS * GRID_ROWS;

/// Menu state — `tvui.py` `config.menu_state` değerlerinin tip-güvenli karşılığı.
/// İndirme listede kalır (kuyruk + satır marker); ayrı progress sayfası YOK.
/// Kuyruk ayrı ekrandır (`Queue`, Q tuşu — duraklat/sürdür buradan).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuState {
    Loading,
    PlatformGrid,
    GameList,
    Queue,
    Error(String),
    ConfirmExit,
}

impl Default for MenuState {
    fn default() -> Self {
        MenuState::Loading
    }
}

/// Oyun satırı (game_list kaynağı). `manager-http` `/api/games` yanıtından dolar.
#[derive(Debug, Clone, Default)]
pub struct GameRow {
    pub name: String,
    pub size: String,
    pub url: String,
    /// Dosya uzantısı (`.zip`, tablo Ext kolonu; addan türetilir).
    pub ext: String,
}

/// SDL'siz ekran state'i — `TvuiState` (SSE/loading) + menu + seçim + key-repeat.
#[derive(Debug, Clone)]
pub struct TvuiScreen {
    pub menu: MenuState,
    /// Platform ızgarası (TvuiState.platforms ile senkron tutulur).
    pub platforms: Vec<PlatformTile>,
    pub selected_platform: usize,
    /// Seçili platformun oyun listesi (net.games ile senkron).
    pub games: Vec<GameRow>,
    /// İstenen platform klasörü (`folder`). `net.games_platform` ile
    /// eşleşmeyen veri ekrana ASLA kopyalanmaz (bayat liste koruması).
    pub games_platform: String,
    pub selected_game: usize,
    /// Sayfa adımı (Python `config.visible_games` parity — default 15,
    /// `config.py:494`; Python'da da layout'tan güncellenmez, sabit).
    pub visible_games: usize,
    /// Faz 4: canlı progress haritası (net.progress ile senkron).
    pub progress: HashMap<String, serde_json::Value>,
    /// WebUI parity (`gameStatuses`): indirilen oyun anahtarları
    /// (`net.downloaded` ile senkron; satır `[>]` marker'ı).
    pub downloaded: HashSet<String>,
    /// Faz Q: kuyruk ekranı listesi (`net.queue` ile senkron).
    pub queue: Vec<crate::net::QueueRow>,
    pub queue_selected: usize,
    /// Kuyruktan dönüş hedefi (girilen ekran — Grid ya da GameList).
    pub queue_from: MenuState,
    /// Son kuyruk çekme anı (5 sn oto-tazeleme eşiği shell'de).
    pub queue_fetched_at: Option<Instant>,
    /// Kuyruk son aksiyon geri bildirimi (duraklat/sürdür sonucu).
    pub queue_note: String,
    /// Faz 5: platform seçim transition'ı (scale+alpha, theme.json ile).
    pub transition: Option<Transition>,
    /// TASK-012i: menü overlay (pause/display/filter/sort/search).
    pub overlay: Option<MenuNav>,
    /// Filter/sort state (game_list filtering parity, display/menus.py)
    pub filters: HashMap<String, String>,
    pub sort_mode: String,
    /// TASK-012j: sanal klavye + folder browser (display/virtual_keyboard.py + folder_browser.py)
    pub keyboard: Option<VirtualKeyboard>,
    pub browser: Option<FolderBrowser>,
    /// Global search query (keyboard.input senkronu, controls/search.py parity)
    pub search_query: String,
    /// TASK-012k: erişilebilirlik (font_scale / footer_font_scale / yüksek kontrast)
    pub a11y: Accessibility,
    /// SSE/loading tarafı (net::TvuiState ile senkron).
    pub net: TvuiState,
    /// Son key-repeat zaman damgası (Python `process_key_repeats` parity).
    last_key: Option<UiKey>,
    last_at: Option<Instant>,
}

impl Default for TvuiScreen {
    fn default() -> Self {
        Self {
            menu: MenuState::Loading,
            platforms: Vec::new(),
            selected_platform: 0,
            games: Vec::new(),
            games_platform: String::new(),
            selected_game: 0,
            visible_games: 15,
            progress: HashMap::new(),
            downloaded: HashSet::new(),
            queue: Vec::new(),
            queue_selected: 0,
            queue_from: MenuState::PlatformGrid,
            queue_fetched_at: None,
            queue_note: String::new(),
            transition: None,
            overlay: None,
            filters: HashMap::new(),
            sort_mode: "name_asc".to_string(),
            keyboard: None,
            browser: None,
            search_query: String::new(),
            a11y: Accessibility::default(),
            net: TvuiState::default(),
            last_key: None,
            last_at: None,
        }
    }
}

impl TvuiScreen {
    pub fn new(net: TvuiState) -> Self {
        let mut s = Self::default();
        s.net = net;
        s.sync_from_net();
        s
    }

    /// `net` tarafındaki loading/ready/error/offline'a göre menu'yu senkronlar.
    /// Loading bar kaynağı SSE `catalog_update`; burası her frame çağrılabilir (idempotent).
    pub fn sync_from_net(&mut self) {
        if self.net.error.is_some() && !self.net.offline {
            if !matches!(self.menu, MenuState::Error(_)) {
                self.menu = MenuState::Error(self.net.error.clone().unwrap_or_default());
            }
        } else if self.net.ready {
            if matches!(self.menu, MenuState::Loading | MenuState::Error(_)) {
                self.menu = MenuState::PlatformGrid;
                // Platformlar net'ten geldiyse kopyala
                if !self.net.platforms.is_empty() && self.platforms.is_empty() {
                    self.platforms = self.net.platforms.clone();
                }
            }
        } else if self.net.loading {
            self.menu = MenuState::Loading;
        }
        // Faz 4: oyun listesi ve progress senkronu (net → screen).
        // Platform el sıkışması: yalnız İSTENEN platformun verisi kopyalanır.
        // Böylece platform değişiminde önceki platformun listesi/marker'ı
        // bir kare bile gösterilmez; çekme bitmeden liste boş + "yükleniyor"dur.
        if self.net.games_ready && self.net.games_platform == self.games_platform {
            self.games = self
                .net
                .games
                .iter()
                .map(|g| GameRow {
                    name: g.name.clone(),
                    size: g.size.clone(),
                    url: g.url.clone(),
                    ext: g.ext.clone(),
                })
                .collect();
            if self.selected_game >= self.games.len() {
                self.selected_game = 0;
            }
        }
        if !self.net.progress.is_empty() {
            self.progress = self.net.progress.clone();
        }
        // İndirilen anahtarları yalnız taze + eşleşen çekmede kopyala.
        if self.net.statuses_ready && self.net.statuses_platform == self.games_platform {
            self.downloaded = self.net.downloaded.clone();
        }
        // Faz Q: kuyruk globaldir (platform el sıkışması yok); taze çekmede kopyala.
        if self.net.queue_ready {
            self.queue = self.net.queue.clone();
            if self.queue_selected >= self.queue.len() {
                self.queue_selected = 0;
            }
            self.queue_note = self.net.queue_note.clone();
        }
    }

    /// Oyun listesi çekme sürüyor mu? (boş liste + taze veri yok = yükleniyor;
    /// boş liste + taze veri = gerçekten oyun yok).
    pub fn games_loading(&self) -> bool {
        self.games.is_empty()
            && !(self.net.games_ready && self.net.games_platform == self.games_platform)
    }

    /// Filtrelenmiş + sıralanmış oyun listesi (display/menus.py parity, SDL'siz).
    /// `filters` map'indeki `filter_usa` gibi anahtarlar `exclude` ise bölge içeren oyun gizlenir.
    /// `sort_mode` `name_asc/desc` veya `size_asc/desc` (size parse sayısal).
    /// Not: `draw_game_list` henüz ham `games` sırasını çizer (katalog sırası);
    /// bu fonksiyon overlay filtre menüsü + ileride liste çizimi için hazırdır.
    pub fn filtered_games(&self) -> Vec<GameRow> {
        let mut list = self.games.clone();
        // TASK-012j: search_query ile alt dize filtresi (controls/search.py filter_games_by_search_query parity)
        if !self.search_query.trim().is_empty() {
            let q = self.search_query.to_lowercase();
            list.retain(|g| g.name.to_lowercase().contains(&q));
        }
        // Filter: region bazlı basit (USA/Europe/Japan/Other)
        for (k, v) in &self.filters {
            if v == "exclude" {
                let region = match k.as_str() {
                    "filter_usa" => "USA",
                    "filter_europe" => "Europe",
                    "filter_japan" => "Japan",
                    "filter_other" => "Other",
                    _ => continue,
                };
                if region == "Other" {
                    // Other: bölgesiz oyunlar (parantez içi bölge yok)
                    list.retain(|g| {
                        let has_region = g.name.contains("(USA)")
                            || g.name.contains("(Europe)")
                            || g.name.contains("(Japan)");
                        !has_region
                    });
                } else {
                    let pat = format!("({region})");
                    list.retain(|g| !g.name.contains(&pat));
                }
            }
        }
        // Sort
        fn parse_size(s: &str) -> u64 {
            s.trim()
                .replace(|c: char| !c.is_ascii_digit(), "")
                .parse::<u64>()
                .unwrap_or(0)
        }
        match self.sort_mode.as_str() {
            "name_desc" => list.sort_by(|a, b| b.name.cmp(&a.name)),
            "size_asc" => list.sort_by(|a, b| parse_size(&a.size).cmp(&parse_size(&b.size))),
            "size_desc" => list.sort_by(|a, b| parse_size(&b.size).cmp(&parse_size(&a.size))),
            _ => list.sort_by(|a, b| a.name.cmp(&b.name)), // name_asc default
        }
        list
    }

    /// Key-repeat filtresi: aynı tuş 120ms içinde tekrar ederse yutulur
    /// (Python `process_key_repeats` 100ms civarı; 120ms güvenli eşik).
    fn is_repeat_throttled(&mut self, key: UiKey, now: Instant) -> bool {
        const THROTTLE: Duration = Duration::from_millis(120);
        if self.last_key == Some(key) {
            if let Some(at) = self.last_at {
                if now.duration_since(at) < THROTTLE {
                    return true;
                }
            }
        }
        self.last_key = Some(key);
        self.last_at = Some(now);
        false
    }

    /// TASK-012j: sanal klavyeyi aç (variant env'den, nintendo_layout flag dahil).
    pub fn open_keyboard(&mut self, variant: KeyboardVariant) {
        let nintendo = std::env::var("RGSX_NINTENDO_LAYOUT").map(|v| v == "1").unwrap_or(false);
        let mut kb = VirtualKeyboard::new(variant, nintendo);
        kb.input = self.search_query.clone();
        self.keyboard = Some(kb);
    }

    pub fn close_keyboard(&mut self) {
        if let Some(kb) = self.keyboard.take() {
            self.search_query = kb.input.clone();
        }
    }

    /// TASK-012j: folder browser aç.
    pub fn open_browser(&mut self, mode: BrowserMode, path: impl Into<std::path::PathBuf>) {
        let mut fb = FolderBrowser::new(mode, path);
        fb.refresh_from_fs();
        self.browser = Some(fb);
    }

    pub fn close_browser(&mut self) {
        self.browser = None;
    }
}

/// WebUI parity (`catalogStatus`, `App.vue:382`): satır durum göstergesi.
/// Öncelik: indirildi `[>]` (yeşil) > aktif `[~] %` (sarı) > başarısız `[X]`
/// (kırmızı) > yok. `downloaded` anahtarları `game_stem`/küçük harf formundadır;
/// `failed` progress `status` metninden okunur (`FAILED[_PERMANENT]`/`ERROR`/`ERREUR`).
/// Dönüş: `Some("[>]")` / `Some("[~] 55%")` / `Some("[X]")` ya da `None`.
pub fn game_marker(
    game_name: &str,
    progress: Option<&serde_json::Value>,
    downloaded: &HashSet<String>,
) -> Option<String> {
    let stem = crate::net::game_stem(game_name);
    if downloaded.contains(&stem) || downloaded.contains(&game_name.to_lowercase()) {
        return Some("[>]".to_string());
    }
    if let Some(marker) = active_download_marker(progress) {
        return Some(marker);
    }
    if let Some(v) = progress {
        let code = v
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        if matches!(
            code.as_str(),
            "FAILED" | "FAILED_PERMANENT" | "ERROR" | "ERREUR"
        ) {
            return Some("[X]".to_string());
        }
    }
    None
}

/// WebUI parity (`catalogStatus` aktif dalı, `App.vue`):
/// yalnız aktif indirme durumlarında satır marker'ı üretir —
/// kuyrukta bekleyen (`Queued`) boş bar illüzyonu vermez.
/// Dönüş: `Some("[~] 55%")` ya da `None` (marker yok).
pub fn active_download_marker(p: Option<&serde_json::Value>) -> Option<String> {
    const ACTIVE: [&str; 5] = [
        "Downloading",
        "Extracting",
        "Connecting",
        "Verifying",
        "Seeding",
    ];
    let v = p?;
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("");
    if !ACTIVE.contains(&status) {
        return None;
    }
    let pct = v
        .get("progress")
        .and_then(|x| x.as_f64())
        .map(|f| f.clamp(0.0, 100.0) as i32)
        .unwrap_or(0);
    Some(format!("[~] {pct}%"))
}

/// Dürüst menü sözleşmesi (footer'da yazan her tuş GERÇEKTEN çalışır):
/// - M / AltGr → pause menüsü (sürdür, görünüm, filtre, sıralama, arama, çıkış)
/// - F (oyun listesi) → arama overlay'i (sorgu listeyi anında filtreler)
/// - H (geçmiş) YOKTUR — geçmiş ekranı henüz yok, footer'da da yazmaz.
fn open_overlay(screen: &mut TvuiScreen, kind: MenuKind) {
    let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
    let en = crate::i18n::load_lang("en");
    screen.overlay = Some(MenuNav::new(kind, &lang, &en));
}

/// SAF reducer: mevcut screen + semantik tuş → (menu geçişi + opsiyonel UiAction).
/// HTTP/SDL içermez; tüm geçiş kuralları burada, unit-test edilir.
pub fn reduce(screen: &mut TvuiScreen, key: UiKey, now: Instant) -> Option<UiAction> {
    // Nav/page tuşlarında key-repeat throttling uygula
    let is_nav = matches!(
        key,
        UiKey::NavUp | UiKey::NavDown | UiKey::NavLeft | UiKey::NavRight | UiKey::PageUp | UiKey::PageDown
    );
    if is_nav && screen.is_repeat_throttled(key, now) {
        return None;
    }
    // Confirm/Retry/CancelUpdate gibi tekil tuşlarda throttle yok — hemen işle
    // ama nav dışı tuşlar last_key'i sıfırlar (yön değiştirince hemen hareket).
    if !is_nav {
        screen.last_key = None;
        screen.last_at = None;
    }

    // TASK-012j: folder browser en üst öncelik (gamepad imleç gezinir, Confirm girer, Back yukarı)
    if screen.browser.is_some() {
        match key {
            UiKey::NavUp => { if let Some(b) = screen.browser.as_mut() { b.nav_up(); } },
            UiKey::NavDown => { if let Some(b) = screen.browser.as_mut() { b.nav_down(); } },
            UiKey::PageUp => { if let Some(b) = screen.browser.as_mut() { b.page_up(); } },
            UiKey::PageDown => { if let Some(b) = screen.browser.as_mut() { b.page_down(); } },
            UiKey::NavLeft => { if let Some(b) = screen.browser.as_mut() { b.nav_up(); } },
            UiKey::NavRight => { if let Some(b) = screen.browser.as_mut() { b.nav_down(); } },
            UiKey::Confirm => {
                if let Some(b) = screen.browser.as_mut() {
                    let _next = b.enter();
                    b.refresh_from_fs();
                }
            }
            UiKey::Back | UiKey::Menu => {
                // Kökte ise browser'ı kapat, değilse parent'a
                let at_root = screen.browser.as_ref().map(|b| b.current_path.parent().is_none()).unwrap_or(true);
                if at_root || key == UiKey::Menu {
                    screen.browser = None;
                } else if let Some(b) = screen.browser.as_mut() {
                    let _up = b.go_parent();
                    b.refresh_from_fs();
                }
            }
            _ => {}
        }
        return None;
    }

    // TASK-012j: sanal klavye açıkken gamepad imleç + Confirm/Back (karakter ekle/sil)
    if screen.keyboard.is_some() {
        match key {
            UiKey::NavUp => { if let Some(kb) = screen.keyboard.as_mut() { kb.move_up(); } },
            UiKey::NavDown => { if let Some(kb) = screen.keyboard.as_mut() { kb.move_down(); } },
            UiKey::NavLeft => { if let Some(kb) = screen.keyboard.as_mut() { kb.move_left(); } },
            UiKey::NavRight => { if let Some(kb) = screen.keyboard.as_mut() { kb.move_right(); } },
            UiKey::PageUp => { if let Some(kb) = screen.keyboard.as_mut() { kb.move_up(); } },
            UiKey::PageDown => { if let Some(kb) = screen.keyboard.as_mut() { kb.move_down(); } },
            UiKey::Confirm => {
                if let Some(kb) = screen.keyboard.as_mut() {
                    let q = kb.confirm();
                    screen.search_query = q;
                }
            }
            UiKey::Back => {
                if let Some(kb) = screen.keyboard.as_mut() {
                    if kb.input.is_empty() {
                        // boşta Back → klavyeyi kapat
                        screen.search_query = kb.input.clone();
                        screen.keyboard = None;
                    } else {
                        let q = kb.backspace();
                        screen.search_query = q;
                    }
                }
            }
            UiKey::Menu => {
                // Menu → klavyeyi kapat ve query'yi koru
                if let Some(kb) = screen.keyboard.take() {
                    screen.search_query = kb.input;
                }
            }
            _ => {}
        }
        return None;
    }

    // TASK-012i: overlay açıkken nav/confirm/back overlay'i yönetir (pause/display/filter/sort)
    if screen.overlay.is_some() {
        let mut close = false;
        let mut next_overlay: Option<MenuNav> = None;
        if let Some(ov) = screen.overlay.as_mut() {
            match key {
                UiKey::NavUp => ov.up(),
                UiKey::NavDown => ov.down(),
                UiKey::Confirm => {
                    let key = ov.selected_key().unwrap_or("");
                    match ov.kind {
                        MenuKind::Pause => {
                            if let Some(pa) = crate::menus::pause_action_for(ov) {
                                match pa {
                                    crate::menus::PauseAction::OpenDisplay => {
                                        next_overlay = Some({
                                            let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
                                            let en = crate::i18n::load_lang("en");
                                            MenuNav::new(MenuKind::Display, &lang, &en)
                                        });
                                        close = true;
                                    }
                                    crate::menus::PauseAction::OpenFilter => {
                                        let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
                                        let en = crate::i18n::load_lang("en");
                                        next_overlay = Some(MenuNav::new(MenuKind::FilterMain, &lang, &en));
                                        close = true;
                                    }
                                    crate::menus::PauseAction::OpenSort => {
                                        let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
                                        let en = crate::i18n::load_lang("en");
                                        next_overlay = Some(MenuNav::new(MenuKind::GlobalSort, &lang, &en));
                                        close = true;
                                    }
                                    crate::menus::PauseAction::Quit => {
                                        screen.menu = MenuState::ConfirmExit;
                                        close = true;
                                    }
                                    _ => close = true,
                                }
                            } else {
                                close = true;
                            }
                        }
                        MenuKind::FilterMain => {
                            match key {
                                // Alt menüye in / geri çık / sıfırla-kapat.
                                "filter_advanced" => {
                                    let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
                                    let en = crate::i18n::load_lang("en");
                                    next_overlay = Some(MenuNav::new(MenuKind::FilterAdvanced, &lang, &en));
                                    close = true;
                                }
                                "filter_back" | "filter_reset" => {
                                    crate::menus::apply_filter_key(key, &mut screen.filters);
                                    close = true;
                                }
                                _ => {
                                    crate::menus::apply_filter_key(key, &mut screen.filters);
                                    // bölge anahtarında açık kal (çoklu toggle)
                                }
                            }
                        }
                        MenuKind::FilterAdvanced => {
                            match key {
                                // Üst menüye dön; bölge anahtarında açık kal.
                                "filter_back" => {
                                    let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
                                    let en = crate::i18n::load_lang("en");
                                    next_overlay = Some(MenuNav::new(MenuKind::FilterMain, &lang, &en));
                                    close = true;
                                }
                                _ => {
                                    crate::menus::apply_filter_key(key, &mut screen.filters);
                                }
                            }
                        }
                        MenuKind::GlobalSort => {
                            screen.sort_mode = match key {
                                "sort_name_asc" => "name_asc",
                                "sort_name_desc" => "name_desc",
                                "sort_size_asc" => "size_asc",
                                "sort_size_desc" => "size_desc",
                                _ => &screen.sort_mode,
                            }
                            .to_string();
                            close = true;
                        }
                        MenuKind::GlobalSearch => {
                            match key {
                                "search_edit" => {
                                    // Klavyeyi aç — overlay kapanır, keyboard aktif olur
                                    let variant = KeyboardVariant::from_str(&std::env::var("RGSX_KEYBOARD_LAYOUT").unwrap_or_else(|_| "qwerty".into()));
                                    screen.open_keyboard(variant);
                                    close = true;
                                }
                                "search_clear" => {
                                    screen.search_query.clear();
                                    if let Some(kb) = screen.keyboard.as_mut() { kb.input.clear(); }
                                    // overlay açık kalır, tekrar arama yapılabilir
                                }
                                _ => close = true,
                            }
                        }
                        MenuKind::Display => {
                            match key {
                                "display_font" => {
                                    screen.a11y.inc_font_scale();
                                    // canlı uygula — kapatma yok, tekrar basıldıkça artar
                                }
                                "display_grid" => {
                                    screen.a11y.inc_footer_scale();
                                }
                                "display_theme" => {
                                    screen.a11y.toggle_high_contrast();
                                }
                                _ => close = true,
                            }
                        }
                    }
                }
                UiKey::Back | UiKey::Menu => close = true,
                _ => {}
            }
        }
        if close {
            screen.overlay = None;
        }
        if let Some(nov) = next_overlay {
            screen.overlay = Some(nov);
        }
        return None;
    }
    if key == UiKey::Menu {
        open_overlay(screen, MenuKind::Pause);
        return None;
    }
    if key == UiKey::Search
        && matches!(screen.menu, MenuState::GameList)
    {
        // F: oyun listesinde arama overlay'i (sorgu listeyi filtreler).
        open_overlay(screen, MenuKind::GlobalSearch);
        return None;
    }
    if key == UiKey::QueueView
        && matches!(screen.menu, MenuState::PlatformGrid | MenuState::GameList)
    {
        // Q: kuyruk ekranı (duraklat/sürdür buradan; dönüş girilen ekrana).
        screen.queue_from = screen.menu.clone();
        screen.queue_selected = 0;
        screen.menu = MenuState::Queue;
        return None;
    }

    match screen.menu.clone() {
        MenuState::Loading => {
            // Loading'de yalnız hata/ready sync'i var; tuşlar UiAction'a delege edilir
            // (Retry → RetryCatalog, Confirm → ContinueOffline vb. net::ui_decision'da)
            // Burada nav yok.
            None
        }
        MenuState::PlatformGrid => match key {
            // Izgara 3×4 sayfa modeli (Python `controls/handlers.py` platform dalı parity):
            // sayfa = selected / 12, hücre = sayfa içi indeks. Kenarda sayfa çevrilir,
            // wrap YOK (son sayfa clamp'lenir).
            UiKey::NavLeft => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let n = screen.platforms.len();
                let page = screen.selected_platform / GRID_PER_PAGE;
                let gi = screen.selected_platform % GRID_PER_PAGE;
                let (row, col) = (gi / GRID_COLS, gi % GRID_COLS);
                if col > 0 {
                    screen.selected_platform -= 1;
                } else if page > 0 {
                    screen.selected_platform = (page - 1) * GRID_PER_PAGE + row * GRID_COLS + (GRID_COLS - 1);
                    if screen.selected_platform >= n {
                        screen.selected_platform = n - 1;
                    }
                }
                None
            }
            UiKey::NavRight => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let n = screen.platforms.len();
                let page = screen.selected_platform / GRID_PER_PAGE;
                let gi = screen.selected_platform % GRID_PER_PAGE;
                let (row, col) = (gi / GRID_COLS, gi % GRID_COLS);
                let max_idx = (GRID_PER_PAGE.min(n - page * GRID_PER_PAGE)).saturating_sub(1);
                if col + 1 < GRID_COLS && gi < max_idx {
                    screen.selected_platform += 1;
                } else if (page + 1) * GRID_PER_PAGE < n {
                    screen.selected_platform = (page + 1) * GRID_PER_PAGE + row * GRID_COLS;
                    if screen.selected_platform >= n {
                        screen.selected_platform = n - 1;
                    }
                }
                None
            }
            UiKey::NavUp => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let n = screen.platforms.len();
                let page = screen.selected_platform / GRID_PER_PAGE;
                let gi = screen.selected_platform % GRID_PER_PAGE;
                let col = gi % GRID_COLS;
                if gi >= GRID_COLS {
                    screen.selected_platform -= GRID_COLS;
                } else if page > 0 {
                    screen.selected_platform =
                        (page - 1) * GRID_PER_PAGE + (GRID_ROWS - 1) * GRID_COLS + col;
                    if screen.selected_platform >= n {
                        screen.selected_platform = n - 1;
                    }
                }
                None
            }
            UiKey::NavDown => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let n = screen.platforms.len();
                let page = screen.selected_platform / GRID_PER_PAGE;
                let gi = screen.selected_platform % GRID_PER_PAGE;
                let col = gi % GRID_COLS;
                let max_idx = (GRID_PER_PAGE.min(n - page * GRID_PER_PAGE)).saturating_sub(1);
                if gi + GRID_COLS <= max_idx {
                    screen.selected_platform += GRID_COLS;
                } else if (page + 1) * GRID_PER_PAGE < n {
                    screen.selected_platform = (page + 1) * GRID_PER_PAGE + col;
                    if screen.selected_platform >= n {
                        screen.selected_platform = n - 1;
                    }
                }
                None
            }
            UiKey::PageUp => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let n = screen.platforms.len();
                let page = screen.selected_platform / GRID_PER_PAGE;
                if page > 0 {
                    let gi = screen.selected_platform % GRID_PER_PAGE;
                    let (row, col) = (gi / GRID_COLS, gi % GRID_COLS);
                    screen.selected_platform = (page - 1) * GRID_PER_PAGE + row * GRID_COLS + col;
                    if screen.selected_platform >= n {
                        screen.selected_platform = n - 1;
                    }
                }
                None
            }
            UiKey::PageDown => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let n = screen.platforms.len();
                let page = screen.selected_platform / GRID_PER_PAGE;
                if (page + 1) * GRID_PER_PAGE < n {
                    let gi = screen.selected_platform % GRID_PER_PAGE;
                    let (row, col) = (gi / GRID_COLS, gi % GRID_COLS);
                    screen.selected_platform = (page + 1) * GRID_PER_PAGE + row * GRID_COLS + col;
                    if screen.selected_platform >= n {
                        screen.selected_platform = n - 1;
                    }
                }
                None
            }
            UiKey::Confirm => {
                if screen.platforms.is_empty() {
                    return None;
                }
                // Faz 5: transition başlat (theme.json platform_select)
                screen.transition = Some(Transition::new(now, 1000, 1.5, 2.5));
                // Platform seç → GameList'e geç. Eski liste ANINDA temizlenir +
                // istenen platform kaydedilir; el sıkışma eşleşene kadar liste
                // boş + "yükleniyor" gösterilir (bayat liste asla gösterilmez).
                screen.games_platform = screen
                    .platforms
                    .get(screen.selected_platform)
                    .map(|p| p.folder.clone())
                    .unwrap_or_default();
                screen.games.clear();
                screen.menu = MenuState::GameList;
                screen.selected_game = 0;
                None
            }
            UiKey::Back => {
                // Kök ekranda Back → ConfirmExit
                screen.menu = MenuState::ConfirmExit;
                None
            }
            _ => None,
        },
        MenuState::GameList => match key {
            // Python parity (controls/handlers.py) + WebUI `filteredGames` parity:
            // gezinme HER ZAMAN görünen (filtreli+sıralı) listededir — Up/Down
            // wrap'li ±1, Left≡PageUp (−visible_games), Right≡PageDown (+visible_games, clamp).
            UiKey::NavUp => {
                let n = screen.filtered_games().len();
                if n > 0 {
                    screen.selected_game =
                        (screen.selected_game.min(n - 1) + n - 1) % n;
                }
                None
            }
            UiKey::NavDown => {
                let n = screen.filtered_games().len();
                if n > 0 {
                    screen.selected_game = (screen.selected_game.min(n - 1) + 1) % n;
                }
                None
            }
            UiKey::NavLeft | UiKey::PageUp => {
                let step = screen.visible_games;
                screen.selected_game = screen.selected_game.saturating_sub(step);
                let n = screen.filtered_games().len();
                if n > 0 {
                    screen.selected_game = screen.selected_game.min(n - 1);
                }
                None
            }
            UiKey::NavRight | UiKey::PageDown => {
                let n = screen.filtered_games().len();
                if n > 0 {
                    let step = screen.visible_games;
                    screen.selected_game =
                        (screen.selected_game + step).min(n - 1);
                }
                None
            }
            UiKey::Confirm | UiKey::Queue => {
                // WebUI parity (`downloadGame`): tek-buton indirme —
                // Enter ve X aynı `POST /api/download {url, platform, game_name}`
                // aksiyonunu üretir ve LİSTEDE KALINIR (kuyruğa atıp indirir;
                // ilerleme satır `[~] %` marker'ından izlenir). `platform`
                // görünen ad (platform_name); backend `platform_folder_for`
                // ile klasöre eşler.
                let list = screen.filtered_games();
                if list.is_empty() {
                    return None;
                }
                let g = &list[screen.selected_game.min(list.len() - 1)];
                let plat = screen
                    .platforms
                    .get(screen.selected_platform)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                return Some(UiAction::DownloadGame {
                    url: g.url.clone(),
                    platform: plat,
                    game_name: g.name.clone(),
                });
            }
            UiKey::Back => {
                screen.menu = MenuState::PlatformGrid;
                None
            }
            _ => None,
        },
        MenuState::Queue => match key {
            // Kuyruk ekranı: gezinme wrap'li, P tümü-durdur, R tümü-sürdür,
            // Esc dönüş (girilen ekrana). Liste shell'de tazelenir (5 sn).
            UiKey::NavUp => {
                if !screen.queue.is_empty() {
                    let n = screen.queue.len();
                    screen.queue_selected = (screen.queue_selected.min(n - 1) + n - 1) % n;
                }
                None
            }
            UiKey::NavDown => {
                if !screen.queue.is_empty() {
                    let n = screen.queue.len();
                    screen.queue_selected = (screen.queue_selected.min(n - 1) + 1) % n;
                }
                None
            }
            UiKey::QueuePause => Some(UiAction::QueuePauseAll),
            UiKey::QueueResume => Some(UiAction::QueueResumeAll),
            UiKey::Back => {
                screen.menu = screen.queue_from.clone();
                None
            }
            _ => None,
        },
        MenuState::Error(_) => {
            // Hata ekranında Retry/Confirm net::ui_decision'a delege — burada yalnız Back
            if key == UiKey::Back {
                screen.menu = MenuState::ConfirmExit;
            }
            None
        }
        MenuState::ConfirmExit => match key {
            UiKey::Confirm => {
                // Çıkış onayı — gerçek shutdown sdl2_shell'de tetiklenir
                None
            }
            UiKey::Back => {
                // Vazgeç → önceki menüye dön (basit: PlatformGrid)
                screen.menu = MenuState::PlatformGrid;
                None
            }
            _ => None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::PlatformTile;

    fn now() -> Instant {
        Instant::now()
    }

    fn make_grid(n: usize) -> TvuiScreen {
        let mut s = TvuiScreen::default();
        s.menu = MenuState::PlatformGrid;
        s.platforms = (0..n)
            .map(|i| PlatformTile {
                name: format!("P{i}"),
                folder: format!("p{i}"),
                image: format!("p{i}.png"),
                games_count: 10 + i,
            })
            .collect();
        s.selected_platform = 0;
        s
    }

    #[test]
    fn platform_grid_nav_single_page_no_wrap() {
        // 3 platform (tek satır, tek sayfa): Right/Down satırda ilerler,
        // kenarda wrap YOK (Python parity — sayfa çevrilmez).
        let mut s = make_grid(3);
        reduce(&mut s, UiKey::NavRight, now());
        assert_eq!(s.selected_platform, 1);
        reduce(&mut s, UiKey::NavRight, now() + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 2);
        reduce(&mut s, UiKey::NavRight, now() + Duration::from_millis(400));
        assert_eq!(s.selected_platform, 2); // sağ kenar: kıpırdamaz
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(600));
        assert_eq!(s.selected_platform, 2); // alt satır yok: kıpırdamaz
        reduce(&mut s, UiKey::NavLeft, now() + Duration::from_millis(800));
        assert_eq!(s.selected_platform, 1);
    }

    #[test]
    fn platform_grid_row_nav() {
        // 8 platform (3×4 tek sayfa): Down/Up satır atlar (±3), Left/Right yatay (±1).
        let mut s = make_grid(8);
        reduce(&mut s, UiKey::NavDown, now());
        assert_eq!(s.selected_platform, 3);
        reduce(&mut s, UiKey::NavUp, now() + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 0);
        reduce(&mut s, UiKey::NavRight, now() + Duration::from_millis(400));
        assert_eq!(s.selected_platform, 1);
        reduce(&mut s, UiKey::NavLeft, now() + Duration::from_millis(600));
        assert_eq!(s.selected_platform, 0);
        reduce(&mut s, UiKey::NavLeft, now() + Duration::from_millis(800));
        assert_eq!(s.selected_platform, 0); // sol kenar: wrap yok
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(1000));
        assert_eq!(s.selected_platform, 3);
    }

    #[test]
    fn platform_grid_page_turns() {
        // 25 platform (3 sayfa): sağ kenardan Right → sonraki sayfa,
        // PageDown/PageUp aynı hücreye atlar, son sayfa clamp'lenir.
        let mut s = make_grid(25);
        s.selected_platform = 2; // sayfa 0, satır 0, kol 2 (sağ kenar)
        reduce(&mut s, UiKey::NavRight, now());
        assert_eq!(s.selected_platform, 12); // sayfa 1, satır 0, kol 0
        reduce(&mut s, UiKey::PageDown, now() + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 24); // sayfa 2, aynı hücre clamp (24)
        reduce(&mut s, UiKey::PageDown, now() + Duration::from_millis(400));
        assert_eq!(s.selected_platform, 24); // son sayfa: kıpırdamaz
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(600));
        assert_eq!(s.selected_platform, 24); // alt satır yok + sonraki sayfa yok
        reduce(&mut s, UiKey::PageUp, now() + Duration::from_millis(800));
        assert_eq!(s.selected_platform, 12); // sayfa 1, aynı hücre
        reduce(&mut s, UiKey::NavLeft, now() + Duration::from_millis(1000));
        assert_eq!(s.selected_platform, 2); // kol 0 → önceki sayfa, aynı satır son kol
    }

    #[test]
    fn platform_grid_confirm_goes_to_gamelist() {
        let mut s = make_grid(2);
        reduce(&mut s, UiKey::Confirm, now());
        assert_eq!(s.menu, MenuState::GameList);
        assert_eq!(s.selected_game, 0);
    }

    #[test]
    fn platform_grid_back_goes_to_confirm_exit() {
        let mut s = make_grid(1);
        reduce(&mut s, UiKey::Back, now());
        assert_eq!(s.menu, MenuState::ConfirmExit);
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(200));
        assert_eq!(s.menu, MenuState::PlatformGrid); // vazgeç
    }

    #[test]
    fn gamelist_nav_and_back() {
        let mut s = TvuiScreen::default();
        s.menu = MenuState::GameList;
        s.games = (0..5)
            .map(|i| GameRow {
                name: format!("G{i}"),
                size: "10M".into(),
                url: format!("http://x/{i}"),
                ext: String::new(),
            })
            .collect();
        s.selected_game = 0;
        reduce(&mut s, UiKey::NavDown, now());
        assert_eq!(s.selected_game, 1);
        reduce(&mut s, UiKey::PageDown, now() + Duration::from_millis(200));
        assert_eq!(s.selected_game, 4); // clamp
        // Confirm indirir ama LİSTEDE KALIR (ayrı progress sayfası yok).
        let a = reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(400));
        assert_eq!(s.menu, MenuState::GameList);
        assert!(matches!(a, Some(UiAction::DownloadGame { .. })));
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(600));
        assert_eq!(s.menu, MenuState::PlatformGrid);
    }

    #[test]
    fn gamelist_left_right_alias_page() {
        // Python parity: Left≡PageUp(−visible_games=15), Right≡PageDown(+15);
        // Up/Down wrap'li. 20 oyunla adım gerçekten ayırt edilir (10 değil).
        // Adlar sıfır-dolgulu: `filtered_games` (name_asc) sırası ekleme
        // sırasıyla aynı kalır (sıralama-stabil test).
        let mut s = TvuiScreen::default();
        s.menu = MenuState::GameList;
        s.games = (0..20)
            .map(|i| GameRow {
                name: format!("G{i:02}"),
                size: "10M".into(),
                url: format!("http://x/{i}"),
                ext: String::new(),
            })
            .collect();
        assert_eq!(s.visible_games, 15); // config.py:494 parity
        s.selected_game = 0;
        reduce(&mut s, UiKey::NavRight, now());
        assert_eq!(s.selected_game, 15);
        reduce(&mut s, UiKey::NavRight, now() + Duration::from_millis(200));
        assert_eq!(s.selected_game, 19); // (15+15).min(19)
        reduce(&mut s, UiKey::NavLeft, now() + Duration::from_millis(400));
        assert_eq!(s.selected_game, 4);
        reduce(&mut s, UiKey::NavLeft, now() + Duration::from_millis(600));
        assert_eq!(s.selected_game, 0); // üstte clamp
        reduce(&mut s, UiKey::NavUp, now() + Duration::from_millis(800));
        assert_eq!(s.selected_game, 19); // wrap
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(1000));
        assert_eq!(s.selected_game, 0); // wrap
    }

    #[test]
    fn gamelist_nav_and_confirm_follow_filtered_list() {
        // WebUI parity: filtreliyken gezinme + Confirm görünen listededir.
        let mut s = TvuiScreen::default();
        s.menu = MenuState::GameList;
        s.games = vec![
            GameRow { name: "Zelda (Europe)".into(), size: "1".into(), url: "http://x/z".into(), ext: String::new() },
            GameRow { name: "Mario (USA)".into(), size: "1".into(), url: "http://x/m".into(), ext: String::new() },
            GameRow { name: "Mario Kart (USA)".into(), size: "1".into(), url: "http://x/mk".into(), ext: String::new() },
        ];
        s.search_query = "mario".into();
        // Filtreli: Mario, Mario Kart (name_asc sıralı).
        assert_eq!(s.filtered_games().len(), 2);
        reduce(&mut s, UiKey::NavDown, now());
        assert_eq!(s.selected_game, 1);
        let a = reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(200));
        match a {
            Some(UiAction::DownloadGame { game_name, .. }) => {
                assert_eq!(game_name, "Mario Kart (USA)");
            }
            other => panic!("filtreli Confirm görünen oyunu indirmeli, {other:?}"),
        }
    }

    #[test]
    fn sync_from_net_loading_to_grid() {
        let mut s = TvuiScreen::default();
        s.net.loading = true;
        s.sync_from_net();
        assert_eq!(s.menu, MenuState::Loading);
        s.net.loading = false;
        s.net.ready = true;
        s.net.platforms = vec![PlatformTile {
            name: "NES".into(),
            folder: "nes".into(),
            image: "nes.png".into(),
            games_count: 10,
        }];
        s.sync_from_net();
        assert_eq!(s.menu, MenuState::PlatformGrid);
        assert_eq!(s.platforms.len(), 1);
    }

    #[test]
    fn sync_from_net_error() {
        let mut s = TvuiScreen::default();
        s.net.error = Some("katalog hazirlanamadi: no_source".into());
        s.sync_from_net();
        assert!(matches!(s.menu, MenuState::Error(_)));
        s.net.offline = true;
        s.net.error = Some("x".into());
        s.menu = MenuState::PlatformGrid;
        s.sync_from_net();
        // offline ise Error'e geçme — offline bayrağı korunur
        assert_eq!(s.menu, MenuState::PlatformGrid);
    }

    #[test]
    fn key_repeat_throttling() {
        let mut s = make_grid(5);
        let t0 = now();
        reduce(&mut s, UiKey::NavRight, t0);
        assert_eq!(s.selected_platform, 1);
        // 50ms sonra aynı tuş → throttled (120ms eşik)
        reduce(&mut s, UiKey::NavRight, t0 + Duration::from_millis(50));
        assert_eq!(s.selected_platform, 1); // hareket etmedi
        // 200ms sonra → tekrar hareket
        reduce(&mut s, UiKey::NavRight, t0 + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 2);
        // Farklı tuş hemen işler (throttle sıfırlanır)
        reduce(&mut s, UiKey::Confirm, t0 + Duration::from_millis(210));
        assert_eq!(s.menu, MenuState::GameList);
    }

    #[test]
    fn page_up_down_grid() {
        // 20 platform (2 sayfa): PageDown/PageUp aynı hücreyle sayfa atlar.
        let mut s = make_grid(20);
        s.selected_platform = 5; // sayfa 0, satır 1, kol 2
        reduce(&mut s, UiKey::PageDown, now());
        assert_eq!(s.selected_platform, 17); // sayfa 1, satır 1, kol 2
        reduce(&mut s, UiKey::PageUp, now() + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 5);
        // Tek sayfada PageDown/PageUp kıpırdatmaz.
        let mut s1 = make_grid(10);
        s1.selected_platform = 5;
        reduce(&mut s1, UiKey::PageDown, now() + Duration::from_millis(400));
        assert_eq!(s1.selected_platform, 5);
    }

    #[test]
    fn overlay_menu_open_and_nav() {
        let mut s = make_grid(2);
        s.menu = MenuState::PlatformGrid;
        reduce(&mut s, UiKey::Menu, now());
        assert!(s.overlay.is_some());
        let sel0 = s.overlay.as_ref().unwrap().selected;
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(200));
        assert_eq!(s.overlay.as_ref().unwrap().selected, (sel0 + 1) % s.overlay.as_ref().unwrap().items.len());
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(400));
        assert!(s.overlay.is_none());
        // tekrar Menu aç, Confirm de kapatır
        reduce(&mut s, UiKey::Menu, now() + Duration::from_millis(600));
        assert!(s.overlay.is_some());
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(800));
        assert!(s.overlay.is_none());
    }

    #[test]
    fn filter_and_sort_games() {
        let mut s = TvuiScreen::default();
        s.games = vec![
            GameRow { name: "Game A (USA)".into(), size: "100".into(), url: "a".into(), ext: String::new() },
            GameRow { name: "Game B (Europe)".into(), size: "200".into(), url: "b".into(), ext: String::new() },
            GameRow { name: "Game C (Japan)".into(), size: "50".into(), url: "c".into(), ext: String::new() },
            GameRow { name: "Game D".into(), size: "300".into(), url: "d".into(), ext: String::new() },
        ];
        // USA exclude
        s.filters.insert("filter_usa".into(), "exclude".into());
        let filtered = s.filtered_games();
        assert_eq!(filtered.len(), 3);
        assert!(!filtered.iter().any(|g| g.name.contains("(USA)")));
        // size_desc
        s.sort_mode = "size_desc".into();
        let sorted = s.filtered_games();
        assert_eq!(sorted[0].name, "Game D");
    }

    #[test]
    fn virtual_keyboard_gamepad_typing() {
        let mut s = TvuiScreen::default();
        s.open_keyboard(crate::virtual_keyboard::KeyboardVariant::Qwerty);
        assert!(s.keyboard.is_some());
        let t0 = now();
        // cursor (0,0)=0, right ->1
        reduce(&mut s, UiKey::NavRight, t0);
        assert_eq!(s.keyboard.as_ref().unwrap().cursor, (0,1));
        reduce(&mut s, UiKey::Confirm, t0 + Duration::from_millis(200));
        assert_eq!(s.search_query, "1");
        assert_eq!(s.keyboard.as_ref().unwrap().input, "1");
        reduce(&mut s, UiKey::Back, t0 + Duration::from_millis(400));
        assert_eq!(s.search_query, "");
        // boşta Back → kapanır
        reduce(&mut s, UiKey::Back, t0 + Duration::from_millis(600));
        assert!(s.keyboard.is_none());
    }

    #[test]
    fn global_search_overlay_opens_keyboard() {
        let mut s = make_grid(1);
        s.menu = MenuState::PlatformGrid;
        reduce(&mut s, UiKey::Menu, now());
        // Pause overlay -> select OpenSearch (index 4)
        if let Some(ov) = s.overlay.as_mut() { ov.selected = 4; }
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(200));
        // Şimdi pause kapandı, GlobalSort değil Search için direkt açalım
        let lang = crate::i18n::load_lang("en");
        s.overlay = Some(crate::menus::MenuNav::new(crate::menus::MenuKind::GlobalSearch, &lang, &lang));
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(400)); // search_edit
        assert!(s.keyboard.is_some());
        assert!(s.overlay.is_none());
    }

    #[test]
    fn search_query_filters_games() {
        let mut s = TvuiScreen::default();
        s.games = vec![
            GameRow { name: "Super Mario (USA)".into(), size: "10".into(), url: "a".into(), ext: String::new() },
            GameRow { name: "Zelda (Europe)".into(), size: "20".into(), url: "b".into(), ext: String::new() },
            GameRow { name: "Mario Kart".into(), size: "30".into(), url: "c".into(), ext: String::new() },
        ];
        s.search_query = "mario".into();
        let filtered = s.filtered_games();
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|g| g.name.to_lowercase().contains("mario")));
    }

    #[test]
    fn folder_browser_gamepad_nav() {
        let mut s = TvuiScreen::default();
        s.open_browser(crate::folder_browser::BrowserMode::RomsRoot, "/tmp");
        s.browser.as_mut().unwrap().set_items(vec!["..".into(), "a".into(), "b".into(), "c".into()]);
        s.browser.as_mut().unwrap().visible_items = 2;
        assert_eq!(s.browser.as_ref().unwrap().selection, 0);
        reduce(&mut s, UiKey::NavDown, now());
        assert_eq!(s.browser.as_ref().unwrap().selection, 1);
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(200));
        assert_eq!(s.browser.as_ref().unwrap().selection, 2);
        reduce(&mut s, UiKey::PageDown, now() + Duration::from_millis(400));
        assert_eq!(s.browser.as_ref().unwrap().selection, 3);
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(600));
        // Back parent'a gitti ama browser hâlâ açık (kök değilse)
        assert!(s.browser.is_some());
    }

    #[test]
    fn accessibility_display_menu_controls() {
        let mut s = TvuiScreen::default();
        let lang = crate::i18n::load_lang("en");
        s.overlay = Some(crate::menus::MenuNav::new(crate::menus::MenuKind::Display, &lang, &lang));
        let initial_font = s.a11y.font_scale();
        // display_font -> inc
        s.overlay.as_mut().unwrap().selected = 2; // display_font
        reduce(&mut s, UiKey::Confirm, now());
        assert!(s.a11y.font_scale() > initial_font);
        // display_theme -> toggle high contrast
        s.overlay = Some(crate::menus::MenuNav::new(crate::menus::MenuKind::Display, &lang, &lang));
        s.overlay.as_mut().unwrap().selected = 0; // display_theme
        assert!(!s.a11y.high_contrast);
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(200));
        assert!(s.a11y.high_contrast);
        assert!(s.overlay.is_some()); // display'de kapatma yok, canlı
    }

    #[test]
    fn accessibility_scales_separate() {
        let mut s = TvuiScreen::default();
        s.a11y.set_font_scale_idx(13); // 2.0
        s.a11y.set_footer_scale_idx(0); // 0.7
        assert_eq!(s.a11y.scaled(100), 200);
        assert_eq!(s.a11y.scaled_footer(100), 70);
        assert_ne!(s.a11y.font_scale(), s.a11y.footer_font_scale());
    }

    #[test]
    fn queue_key_aliases_confirm_webui_single_button() {
        // WebUI parity: X (Queue) ve Enter (Confirm) aynı DownloadGame aksiyonu.
        let mut s = TvuiScreen::default();
        s.menu = MenuState::GameList;
        s.platforms = vec![crate::net::PlatformTile {
            name: "SNES".into(),
            folder: "snes".into(),
            image: "snes.png".into(),
            games_count: 1,
        }];
        s.games = vec![GameRow {
            name: "Zelda.zip".into(),
            size: "10M".into(),
            url: "http://x/zelda".into(),
            ext: ".zip".into(),
        }];
        let a = reduce(&mut s, UiKey::Queue, now());
        assert_eq!(s.menu, MenuState::GameList); // listede kalır
        match a {
            Some(UiAction::DownloadGame { url, platform, game_name }) => {
                assert_eq!(url, "http://x/zelda");
                assert_eq!(platform, "SNES");
                assert_eq!(game_name, "Zelda.zip");
            }
            other => panic!("Queue DownloadGame üretmeli, {other:?}"),
        }
    }

    #[test]
    fn active_download_marker_only_when_active() {
        let dl = serde_json::json!({"progress": 55.0, "status": "Downloading"});
        assert_eq!(
            active_download_marker(Some(&dl)),
            Some("[~] 55%".to_string())
        );
        let queued = serde_json::json!({"progress": 0.0, "status": "Queued"});
        assert_eq!(active_download_marker(Some(&queued)), None);
        let done = serde_json::json!({"progress": 100.0, "status": "Download_OK"});
        assert_eq!(active_download_marker(Some(&done)), None);
        assert_eq!(active_download_marker(None), None);
    }

    #[test]
    fn game_marker_priority_downloaded_over_active_over_failed() {
        let empty: HashSet<String> = HashSet::new();
        let active = serde_json::json!({"progress": 55.0, "status": "Downloading"});
        let failed = serde_json::json!({"progress": 10.0, "status": "FAILED"});
        // Aktif → sarı.
        assert_eq!(
            game_marker("Sonic.zip", Some(&active), &empty),
            Some("[~] 55%".to_string())
        );
        // Başarısız → kırmızı.
        assert_eq!(
            game_marker("Sonic.zip", Some(&failed), &empty),
            Some("[X]".to_string())
        );
        // İndirildi her şeyi ezer (yeşil öncelik, WebUI parity).
        let mut dl = HashSet::new();
        dl.insert("sonic".to_string());
        assert_eq!(
            game_marker("Sonic.zip", Some(&active), &dl),
            Some("[>]".to_string())
        );
        assert_eq!(
            game_marker("Sonic.zip", Some(&failed), &dl),
            Some("[>]".to_string())
        );
        // Hiçbiri → marker yok.
        assert_eq!(game_marker("Other.zip", None, &empty), None);
    }

    #[test]
    fn sync_from_net_copies_downloaded_only_when_fresh() {
        // Taze çekme varsa kopyalanır.
        let mut s = TvuiScreen::default();
        s.net.statuses_ready = true;
        s.net.downloaded.insert("sonic".to_string());
        s.sync_from_net();
        assert!(s.downloaded.contains("sonic"));
        // Taze değilse eski veri korunur (hata halinde wipe yok).
        let mut s2 = TvuiScreen::default();
        s2.downloaded.insert("zelda".to_string());
        s2.sync_from_net();
        assert!(s2.downloaded.contains("zelda"));
    }

    #[test]
    fn platform_switch_never_shows_stale_list() {
        // Platform girişi eski listeyi ANINDA temizler + platformu kaydeder.
        let mut s = make_grid(2);
        s.games = vec![GameRow {
            name: "Eski".into(), size: "1".into(), url: "x".into(), ext: String::new(),
        }];
        s.selected_platform = 1; // folder p1
        reduce(&mut s, UiKey::Confirm, now());
        assert_eq!(s.menu, MenuState::GameList);
        assert!(s.games.is_empty(), "bayat liste temizlenmeli");
        assert_eq!(s.games_platform, "p1");
        // Ağda hâlâ eski platform verisi: kopyalanMAZ, yükleniyor gösterilir.
        s.net.games = vec![crate::net::GameRow {
            name: "Eski".into(), size: "1".into(), url: "x".into(), ext: String::new(),
        }];
        s.net.games_platform = "p0".to_string();
        s.net.games_ready = true;
        s.sync_from_net();
        assert!(s.games.is_empty());
        assert!(s.games_loading());
        // Eşleşen çekme gelince kopyalanır (boş bile olsa — gerçekten oyun yok).
        s.net.games.clear();
        s.net.games_platform = "p1".to_string();
        s.sync_from_net();
        assert!(!s.games_loading());
        s.net.games = vec![crate::net::GameRow {
            name: "Yeni".into(), size: "2".into(), url: "y".into(), ext: String::new(),
        }];
        s.sync_from_net();
        assert_eq!(s.games.len(), 1);
        assert_eq!(s.games[0].name, "Yeni");
        // Başka platformun marker'ı bulaşmaz.
        s.net.downloaded.insert("eski".to_string());
        s.net.statuses_platform = "p0".to_string();
        s.net.statuses_ready = true;
        s.sync_from_net();
        assert!(!s.downloaded.contains("eski"));
        s.net.statuses_platform = "p1".to_string();
        s.sync_from_net();
        assert!(s.downloaded.contains("eski"));
    }

    #[test]
    fn pause_display_and_quit_are_real() {
        // Display girdisi Display alt menüsünü AÇAR (eskiden sessizce kapanırdı).
        let mut s = make_grid(1);
        reduce(&mut s, UiKey::Menu, now());
        s.overlay.as_mut().unwrap().selected = 1; // pause_display
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(200));
        let ov = s.overlay.as_ref().expect("display açılmalı");
        assert_eq!(ov.kind, MenuKind::Display);
        // Quit çıkış onayına gider (kapanıp yutmaz).
        let mut q = make_grid(1);
        reduce(&mut q, UiKey::Menu, now());
        q.overlay.as_mut().unwrap().selected = 5; // pause_quit
        reduce(&mut q, UiKey::Confirm, now() + Duration::from_millis(200));
        assert!(q.overlay.is_none());
        assert_eq!(q.menu, MenuState::ConfirmExit);
    }

    #[test]
    fn filter_submenu_navigation_is_real() {
        // filter_advanced alt menüye iner, filter_back geri çıkar.
        let mut s = make_grid(1);
        reduce(&mut s, UiKey::Menu, now());
        s.overlay.as_mut().unwrap().selected = 2; // pause_filter
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(200));
        assert_eq!(s.overlay.as_ref().unwrap().kind, MenuKind::FilterMain);
        // items: region, advanced, reset, back → advanced index 1
        s.overlay.as_mut().unwrap().selected = 1;
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(400));
        assert_eq!(s.overlay.as_ref().unwrap().kind, MenuKind::FilterAdvanced);
        // advanced items: usa, europe, japan, other, back → back index 4
        s.overlay.as_mut().unwrap().selected = 4;
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(600));
        assert_eq!(s.overlay.as_ref().unwrap().kind, MenuKind::FilterMain);
        // reset temizler VE kapatır.
        s.filters.insert("filter_usa".into(), "exclude".into());
        s.overlay.as_mut().unwrap().selected = 2; // filter_reset
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(800));
        assert!(s.filters.is_empty());
        assert!(s.overlay.is_none());
    }

    #[test]
    fn search_key_opens_search_only_in_gamelist() {
        // F oyun listesinde arama overlay'i açar, grid'de yoksayılır.
        let mut s = TvuiScreen::default();
        s.menu = MenuState::GameList;
        reduce(&mut s, UiKey::Search, now());
        assert_eq!(s.overlay.as_ref().map(|o| o.kind.clone()), Some(MenuKind::GlobalSearch));
        let mut g = make_grid(1);
        reduce(&mut g, UiKey::Search, now());
        assert!(g.overlay.is_none());
    }

    #[test]
    fn queue_view_enter_back_and_actions() {
        // Q grid'den kuyruğa girer, Esc geri döner (girilen ekrana).
        let mut s = make_grid(2);
        reduce(&mut s, UiKey::QueueView, now());
        assert_eq!(s.menu, MenuState::Queue);
        assert_eq!(s.queue_from, MenuState::PlatformGrid);
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(200));
        assert_eq!(s.menu, MenuState::PlatformGrid);
        // Oyun listesinden girişte dönüş listeye.
        let mut g = TvuiScreen::default();
        g.menu = MenuState::GameList;
        reduce(&mut g, UiKey::QueueView, now());
        assert_eq!(g.queue_from, MenuState::GameList);
        reduce(&mut g, UiKey::Back, now() + Duration::from_millis(200));
        assert_eq!(g.menu, MenuState::GameList);
        // P/R aksiyon üretir (arka plan HTTP shell'de).
        let mut q = TvuiScreen::default();
        q.menu = MenuState::Queue;
        assert_eq!(
            reduce(&mut q, UiKey::QueuePause, now()),
            Some(UiAction::QueuePauseAll)
        );
        assert_eq!(
            reduce(&mut q, UiKey::QueueResume, now() + Duration::from_millis(200)),
            Some(UiAction::QueueResumeAll)
        );
        assert_eq!(q.menu, MenuState::Queue); // ekranda kalır
    }

    #[test]
    fn queue_nav_wraps_and_sync_copies_when_ready() {
        let mut s = TvuiScreen::default();
        s.menu = MenuState::Queue;
        s.queue = vec![
            crate::net::QueueRow { name: "A".into(), url: "u1".into(), status: "Queued".into() },
            crate::net::QueueRow { name: "B".into(), url: "u2".into(), status: "Downloading".into() },
        ];
        reduce(&mut s, UiKey::NavUp, now());
        assert_eq!(s.queue_selected, 1); // wrap
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(200));
        assert_eq!(s.queue_selected, 0);
        // Senkron: hazır değilse eski liste korunur.
        let mut n = TvuiScreen::default();
        n.queue = vec![crate::net::QueueRow {
            name: "Eski".into(), url: "ux".into(), status: "Queued".into(),
        }];
        n.sync_from_net();
        assert_eq!(n.queue.len(), 1);
        // Hazırsa kopyalanır + seçim clamp'lenir.
        n.net.queue_ready = true;
        n.net.queue = vec![];
        n.queue_selected = 5;
        n.sync_from_net();
        assert!(n.queue.is_empty());
        assert_eq!(n.queue_selected, 0);
    }
}
