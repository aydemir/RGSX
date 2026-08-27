//! TASK-012i — SDL2 menüler (pause / display / filter / sort / search) SDL'siz çekirdek.
//!
//! `display/menus.py` (92KB) + `global_search.py` parity: pause/display/filter/sort/search
//! menülerinin state machine'i ve etiket/i18n eşlemesi. SDL yalnız piksel; karar burada.

use crate::i18n::{t_with_fallback, LangMap};

/// Menü türü.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuKind {
    Pause,
    Display,
    FilterMain,
    FilterAdvanced,
    GlobalSort,
    GlobalSearch,
}

/// Menü navigasyon state'i (SDL'siz).
#[derive(Debug, Clone)]
pub struct MenuNav {
    pub kind: MenuKind,
    pub selected: usize,
    pub items: Vec<String>, // etiket key'leri (i18n key)
}

impl MenuNav {
    pub fn new(kind: MenuKind, lang: &LangMap, fallback: &LangMap) -> Self {
        let keys = menu_keys(&kind);
        let items = keys.iter().map(|k| t_with_fallback(k, lang, fallback)).collect();
        Self {
            kind,
            selected: 0,
            items,
        }
    }

    pub fn up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        } else if !self.items.is_empty() {
            self.selected = self.items.len() - 1;
        }
    }

    pub fn down(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
    }

    pub fn selected_label(&self) -> Option<&str> {
        self.items.get(self.selected).map(|s| s.as_str())
    }

    pub fn selected_key(&self) -> Option<&'static str> {
        let keys = menu_keys(&self.kind);
        keys.get(self.selected).copied()
    }
}

fn menu_keys(kind: &MenuKind) -> Vec<&'static str> {
    match kind {
        MenuKind::Pause => vec!["pause_resume", "pause_display", "pause_filter", "pause_sort", "pause_search", "pause_quit"],
        MenuKind::Display => vec!["display_theme", "display_grid", "display_font", "display_back"],
        MenuKind::FilterMain => vec!["filter_region", "filter_advanced", "filter_reset", "filter_back"],
        MenuKind::FilterAdvanced => vec!["filter_usa", "filter_europe", "filter_japan", "filter_other", "filter_back"],
        MenuKind::GlobalSort => vec!["sort_name_asc", "sort_name_desc", "sort_size_asc", "sort_size_desc", "sort_back"],
        MenuKind::GlobalSearch => vec!["search_edit", "search_clear", "search_back"],
    }
}

/// Pause menü aksiyonu (shell tarafından `apply_ui_action`'a çevrilir).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PauseAction {
    Resume,
    PauseAll,
    OpenDisplay,
    OpenFilter,
    OpenSort,
    OpenSearch,
    Quit,
}

pub fn pause_action_for(nav: &MenuNav) -> Option<PauseAction> {
    if nav.kind != MenuKind::Pause {
        return None;
    }
    match nav.selected_key()? {
        "pause_resume" => Some(PauseAction::Resume),
        "pause_display" => Some(PauseAction::OpenDisplay),
        "pause_filter" => Some(PauseAction::OpenFilter),
        "pause_sort" => Some(PauseAction::OpenSort),
        "pause_search" => Some(PauseAction::OpenSearch),
        "pause_quit" => Some(PauseAction::Quit),
        _ => None,
    }
}

/// Filter/sort seçimini uygular (pure, test edilebilir).
/// `selected_key` → filtre map'ine yazılır; gerçek `game_filters` crate'i ile entegre edilecek.
pub fn apply_filter_key(selected: &str, filters: &mut std::collections::HashMap<String, String>) {
    match selected {
        "filter_usa" | "filter_europe" | "filter_japan" | "filter_other" => {
            let cur = filters.get(selected).cloned().unwrap_or_else(|| "include".into());
            let next = match cur.as_str() {
                "include" => "exclude",
                "exclude" => "include",
                _ => "include",
            };
            filters.insert(selected.to_string(), next.to_string());
        }
        "filter_reset" => {
            filters.clear();
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn en() -> LangMap {
        let mut m = HashMap::new();
        for k in menu_keys(&MenuKind::Pause) {
            m.insert(k.to_string(), k.to_string());
        }
        for k in menu_keys(&MenuKind::Display) {
            m.insert(k.to_string(), k.to_string());
        }
        m
    }

    #[test]
    fn pause_nav_wraps() {
        let mut nav = MenuNav::new(MenuKind::Pause, &en(), &HashMap::new());
        assert_eq!(nav.selected, 0);
        nav.up();
        assert_eq!(nav.selected, nav.items.len() - 1);
        nav.down();
        assert_eq!(nav.selected, 0);
    }

    #[test]
    fn pause_action_mapping() {
        let mut nav = MenuNav::new(MenuKind::Pause, &en(), &HashMap::new());
        // 0: pause_resume
        assert_eq!(pause_action_for(&nav), Some(PauseAction::Resume));
        nav.selected = 1;
        assert_eq!(pause_action_for(&nav), Some(PauseAction::OpenDisplay));
        nav.selected = 5;
        assert_eq!(pause_action_for(&nav), Some(PauseAction::Quit));
    }

    #[test]
    fn display_menu_keys() {
        let nav = MenuNav::new(MenuKind::Display, &en(), &HashMap::new());
        assert!(nav.items.contains(&"display_theme".to_string()));
        assert!(nav.items.contains(&"display_back".to_string()));
    }

    #[test]
    fn filter_toggle() {
        let mut filters = HashMap::new();
        apply_filter_key("filter_usa", &mut filters);
        assert_eq!(filters.get("filter_usa").map(|s| s.as_str()), Some("exclude"));
        apply_filter_key("filter_usa", &mut filters);
        assert_eq!(filters.get("filter_usa").map(|s| s.as_str()), Some("include"));
        apply_filter_key("filter_reset", &mut filters);
        assert!(filters.is_empty());
    }

    #[test]
    fn global_sort_keys() {
        let nav = MenuNav::new(MenuKind::GlobalSort, &en(), &HashMap::new());
        assert_eq!(nav.items.len(), 5);
        assert_eq!(nav.selected_key(), Some("sort_name_asc"));
    }

    #[test]
    fn i18n_fallback() {
        let mut primary = HashMap::new();
        primary.insert("pause_resume".into(), "Devam".into());
        let mut fallback = HashMap::new();
        fallback.insert("pause_quit".into(), "Quit".into());
        let nav = MenuNav::new(MenuKind::Pause, &primary, &fallback);
        assert_eq!(nav.items[0], "Devam");
        assert_eq!(nav.items[5], "Quit");
    }
}
