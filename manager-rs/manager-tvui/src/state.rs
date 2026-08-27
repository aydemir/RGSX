//! TASK-012h Faz 1 — SDL'siz çekirdek state machine + SAF input reducer.
//!
//! `tvui.py` `config.menu_state` dispatch'inin tip-güvenli Rust karşılığı.
//! SDL yalnız piksel işi yapar; karar/test edilebilir her şey burada, SDL'siz.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::menus::{MenuKind, MenuNav};
use crate::net::{PlatformTile, TvuiState, UiAction, UiKey};
use crate::render::Transition;

/// Menu state — `tvui.py` `config.menu_state` değerlerinin tip-güvenli karşılığı.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuState {
    Loading,
    PlatformGrid,
    GameList,
    Progress,
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
    pub selected_game: usize,
    /// Faz 4: canlı progress haritası (net.progress ile senkron).
    pub progress: HashMap<String, serde_json::Value>,
    /// Faz 5: platform seçim transition'ı (scale+alpha, theme.json ile).
    pub transition: Option<Transition>,
    /// TASK-012i: menü overlay (pause/display/filter/sort/search).
    pub overlay: Option<MenuNav>,
    /// Filter/sort state (game_list filtering parity, display/menus.py)
    pub filters: HashMap<String, String>,
    pub sort_mode: String,
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
            selected_game: 0,
            progress: HashMap::new(),
            transition: None,
            overlay: None,
            filters: HashMap::new(),
            sort_mode: "name_asc".to_string(),
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
        // Faz 4: oyun listesi ve progress senkronu (net → screen)
        if !self.net.games.is_empty() {
            self.games = self
                .net
                .games
                .iter()
                .map(|g| GameRow {
                    name: g.name.clone(),
                    size: g.size.clone(),
                    url: g.url.clone(),
                })
                .collect();
            if self.selected_game >= self.games.len() {
                self.selected_game = 0;
            }
        }
        if !self.net.progress.is_empty() {
            self.progress = self.net.progress.clone();
        }
    }

    /// Filtrelenmiş + sıralanmış oyun listesi (display/menus.py parity, SDL'siz).
    /// `filters` map'indeki `filter_usa` gibi anahtarlar `exclude` ise bölge içeren oyun gizlenir.
    /// `sort_mode` `name_asc/desc` veya `size_asc/desc` (size parse sayısal).
    pub fn filtered_games(&self) -> Vec<GameRow> {
        let mut list = self.games.clone();
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
                                    _ => close = true,
                                }
                            } else {
                                close = true;
                            }
                        }
                        MenuKind::FilterMain | MenuKind::FilterAdvanced => {
                            crate::menus::apply_filter_key(key, &mut screen.filters);
                            // filtrede kal, kapatma yok (çoklu toggle)
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
                        _ => close = true,
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
        let lang = crate::i18n::load_lang(&crate::i18n::detect_lang());
        let en = crate::i18n::load_lang("en");
        screen.overlay = Some(MenuNav::new(MenuKind::Pause, &lang, &en));
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
            UiKey::NavUp | UiKey::NavLeft => {
                if screen.platforms.is_empty() {
                    return None;
                }
                if screen.selected_platform > 0 {
                    screen.selected_platform -= 1;
                } else {
                    screen.selected_platform = screen.platforms.len() - 1; // wrap
                }
                None
            }
            UiKey::NavDown | UiKey::NavRight => {
                if screen.platforms.is_empty() {
                    return None;
                }
                screen.selected_platform = (screen.selected_platform + 1) % screen.platforms.len();
                None
            }
            UiKey::PageUp => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let step = 6usize; // grid 3×2 varsayımı; test edilebilir sabit
                screen.selected_platform = screen.selected_platform.saturating_sub(step);
                None
            }
            UiKey::PageDown => {
                if screen.platforms.is_empty() {
                    return None;
                }
                let step = 6usize;
                screen.selected_platform =
                    (screen.selected_platform + step).min(screen.platforms.len() - 1);
                None
            }
            UiKey::Confirm => {
                if screen.platforms.is_empty() {
                    return None;
                }
                // Faz 5: transition başlat (theme.json platform_select)
                screen.transition = Some(Transition::new(now, 1000, 1.5, 2.5));
                // Platform seç → GameList'e geç (oyunlar SSE/HTTP ile sonra dolar)
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
            UiKey::NavUp => {
                if screen.selected_game > 0 {
                    screen.selected_game -= 1;
                }
                None
            }
            UiKey::NavDown => {
                if !screen.games.is_empty() {
                    screen.selected_game =
                        (screen.selected_game + 1).min(screen.games.len() - 1);
                }
                None
            }
            UiKey::PageUp => {
                screen.selected_game = screen.selected_game.saturating_sub(10);
                None
            }
            UiKey::PageDown => {
                if !screen.games.is_empty() {
                    screen.selected_game =
                        (screen.selected_game + 10).min(screen.games.len() - 1);
                }
                None
            }
            UiKey::Confirm => {
                // Oyunu indirme tetikle — Progress'e geç (SSE progress akışı) + download action
                if screen.games.is_empty() {
                    return None;
                }
                let g = &screen.games[screen.selected_game];
                let plat = screen
                    .platforms
                    .get(screen.selected_platform)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                screen.menu = MenuState::Progress;
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
        MenuState::Progress => match key {
            UiKey::Back => {
                screen.menu = MenuState::GameList;
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
            })
            .collect();
        s.selected_platform = 0;
        s
    }

    #[test]
    fn platform_grid_nav_wraps() {
        let mut s = make_grid(3);
        reduce(&mut s, UiKey::NavDown, now());
        assert_eq!(s.selected_platform, 1);
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 2);
        reduce(&mut s, UiKey::NavDown, now() + Duration::from_millis(400));
        assert_eq!(s.selected_platform, 0); // wrap
        reduce(&mut s, UiKey::NavUp, now() + Duration::from_millis(600));
        assert_eq!(s.selected_platform, 2); // wrap reverse
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
            })
            .collect();
        s.selected_game = 0;
        reduce(&mut s, UiKey::NavDown, now());
        assert_eq!(s.selected_game, 1);
        reduce(&mut s, UiKey::PageDown, now() + Duration::from_millis(200));
        assert_eq!(s.selected_game, 4); // clamp
        reduce(&mut s, UiKey::Confirm, now() + Duration::from_millis(400));
        assert_eq!(s.menu, MenuState::Progress);
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(600));
        assert_eq!(s.menu, MenuState::GameList);
        reduce(&mut s, UiKey::Back, now() + Duration::from_millis(800));
        assert_eq!(s.menu, MenuState::PlatformGrid);
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
        reduce(&mut s, UiKey::NavDown, t0);
        assert_eq!(s.selected_platform, 1);
        // 50ms sonra aynı tuş → throttled (120ms eşik)
        reduce(&mut s, UiKey::NavDown, t0 + Duration::from_millis(50));
        assert_eq!(s.selected_platform, 1); // hareket etmedi
        // 200ms sonra → tekrar hareket
        reduce(&mut s, UiKey::NavDown, t0 + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 2);
        // Farklı tuş hemen işler (throttle sıfırlanır)
        reduce(&mut s, UiKey::Confirm, t0 + Duration::from_millis(210));
        assert_eq!(s.menu, MenuState::GameList);
    }

    #[test]
    fn page_up_down_grid() {
        let mut s = make_grid(10);
        s.selected_platform = 5;
        reduce(&mut s, UiKey::PageDown, now());
        assert_eq!(s.selected_platform, 9); // clamp
        reduce(&mut s, UiKey::PageUp, now() + Duration::from_millis(200));
        assert_eq!(s.selected_platform, 3);
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
            GameRow { name: "Game A (USA)".into(), size: "100".into(), url: "a".into() },
            GameRow { name: "Game B (Europe)".into(), size: "200".into(), url: "b".into() },
            GameRow { name: "Game C (Japan)".into(), size: "50".into(), url: "c".into() },
            GameRow { name: "Game D".into(), size: "300".into(), url: "d".into() },
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
}
