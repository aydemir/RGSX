//! TASK-012h Faz 1 — SDL'siz çekirdek state machine + SAF input reducer.
//!
//! `tvui.py` `config.menu_state` dispatch'inin tip-güvenli Rust karşılığı.
//! SDL yalnız piksel işi yapar; karar/test edilebilir her şey burada, SDL'siz.

use std::collections::HashMap;
use std::time::{Duration, Instant};

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
}
