//! TASK-012j — Folder browser (display/folder_browser.py parity).
//!
//! Python `config.folder_browser_*` (path, items, selection, scroll_offset,
//! visible_items, platform_config_name, folder_browser_mode) → tip-güvenli struct.
//! SDL yalnız piksel; karar/test edilebilir her şey burada.

use std::path::{Path, PathBuf};

/// Browser modu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowserMode {
    Platform(String), // platform_config_name
    RomsRoot,
    HistoryMove,
}

impl BrowserMode {
    pub fn title(&self) -> String {
        match self {
            BrowserMode::RomsRoot => "Select default ROMs folder".to_string(),
            BrowserMode::HistoryMove => "Select destination folder".to_string(),
            BrowserMode::Platform(name) => format!("Select folder for {name}"),
        }
    }
}

/// Folder browser state — gamepad ile gezinir, Confirm girer, Back yukarı çıkar.
#[derive(Debug, Clone)]
pub struct FolderBrowser {
    pub mode: BrowserMode,
    pub current_path: PathBuf,
    pub items: Vec<String>, // klasör adları + ".." + drive'lar; display/folder_browser.py items
    pub selection: usize,
    pub scroll_offset: usize,
    pub visible_items: usize,
}

impl FolderBrowser {
    pub fn new(mode: BrowserMode, path: impl Into<PathBuf>) -> Self {
        Self {
            mode,
            current_path: path.into(),
            items: Vec::new(),
            selection: 0,
            scroll_offset: 0,
            visible_items: 8, // draw_folder_browser'da dinamik; varsayılan 8
        }
    }

    /// Items'ı dışarıdan doldurur (fs okuma manager-http browse_directories ile sonra).
    pub fn set_items(&mut self, items: Vec<String>) {
        self.items = items;
        self.clamp_selection();
        self.clamp_scroll();
    }

    /// Klasör listesini filesystem'den doldurur (opsiyonel sync helper — test dışı).
    pub fn refresh_from_fs(&mut self) {
        let items = list_dirs(&self.current_path);
        self.set_items(items);
    }

    pub fn visible_slice(&self) -> &[String] {
        let end = (self.scroll_offset + self.visible_items).min(self.items.len());
        &self.items[self.scroll_offset..end]
    }

    pub fn selected_item(&self) -> Option<&str> {
        self.items.get(self.selection).map(|s| s.as_str())
    }

    pub fn is_selected_parent(&self) -> bool {
        self.selected_item() == Some("..")
    }

    pub fn nav_up(&mut self) {
        if self.items.is_empty() { return; }
        if self.selection > 0 {
            self.selection -= 1;
        } else {
            self.selection = self.items.len() - 1;
        }
        self.ensure_visible();
    }

    pub fn nav_down(&mut self) {
        if self.items.is_empty() { return; }
        self.selection = (self.selection + 1) % self.items.len();
        self.ensure_visible();
    }

    pub fn page_up(&mut self) {
        if self.items.is_empty() { return; }
        let step = self.visible_items.max(1);
        self.selection = self.selection.saturating_sub(step);
        self.ensure_visible();
    }

    pub fn page_down(&mut self) {
        if self.items.is_empty() { return; }
        let step = self.visible_items.max(1);
        self.selection = (self.selection + step).min(self.items.len() - 1);
        self.ensure_visible();
    }

    /// Confirm: klasöre girer (".." → parent, drive → drive, dir → child).
    /// Yeni path döndürür; items refresh'i çağıran yapar (fs veya mock).
    pub fn enter(&mut self) -> PathBuf {
        let Some(sel) = self.selected_item().map(|s| s.to_string()) else {
            return self.current_path.clone();
        };
        let next = if sel == ".." {
            self.current_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.current_path.clone())
        } else if sel.len() >= 2 && sel.chars().nth(1) == Some(':') {
            // Windows drive: "C:"
            PathBuf::from(format!("{sel}\\"))
        } else {
            self.current_path.join(&sel)
        };
        self.current_path = next.clone();
        self.selection = 0;
        self.scroll_offset = 0;
        next
    }

    /// Back: bir üst klasöre (enter ".." ile aynı, gamepad Back için).
    pub fn go_parent(&mut self) -> PathBuf {
        let next = self.current_path.parent().map(|p| p.to_path_buf()).unwrap_or_else(|| self.current_path.clone());
        self.current_path = next.clone();
        self.selection = 0;
        self.scroll_offset = 0;
        next
    }

    pub fn set_visible_items(&mut self, n: usize) {
        self.visible_items = n.max(1);
        self.clamp_scroll();
        self.ensure_visible();
    }

    fn clamp_selection(&mut self) {
        if self.items.is_empty() {
            self.selection = 0;
        } else if self.selection >= self.items.len() {
            self.selection = self.items.len() - 1;
        }
    }

    fn clamp_scroll(&mut self) {
        let max_off = self.items.len().saturating_sub(self.visible_items);
        if self.scroll_offset > max_off {
            self.scroll_offset = max_off;
        }
    }

    fn ensure_visible(&mut self) {
        if self.items.is_empty() { self.scroll_offset = 0; return; }
        if self.selection < self.scroll_offset {
            self.scroll_offset = self.selection;
        } else if self.selection >= self.scroll_offset + self.visible_items {
            self.scroll_offset = self.selection + 1 - self.visible_items;
        }
        self.clamp_scroll();
    }
}

/// Filesystem'den klasör listesi: ".." + alt klasörler (sıralı). Drive'lar path boşsa Windows'ta.
fn list_dirs(path: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if path.parent().is_some() {
        out.push("..".to_string());
    }
    let Ok(rd) = std::fs::read_dir(path) else {
        // Boş path veya erişilemez → sürücüleri dene (Windows)
        #[cfg(windows)]
        if path.as_os_str().is_empty() {
            for c in b'A'..=b'Z' {
                let d = format!("{}:", c as char);
                if Path::new(&format!("{d}\\")).exists() {
                    out.push(d);
                }
            }
        }
        return out;
    };
    let mut dirs: Vec<String> = rd.filter_map(|e| {
        let e = e.ok()?;
        let ft = e.file_type().ok()?;
        if ft.is_dir() {
            e.file_name().into_string().ok()
        } else { None }
    }).collect();
    dirs.sort();
    out.extend(dirs);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn browser_with_items(items: Vec<&str>) -> FolderBrowser {
        let mut fb = FolderBrowser::new(BrowserMode::Platform("Test".into()), "/tmp/test");
        fb.set_items(items.into_iter().map(|s| s.to_string()).collect());
        fb.visible_items = 3;
        fb
    }

    #[test]
    fn nav_wraps_and_scroll() {
        let mut fb = browser_with_items(vec!["..", "a", "b", "c", "d"]);
        assert_eq!(fb.selection, 0);
        fb.nav_up();
        assert_eq!(fb.selection, 4); // wrap
        assert_eq!(fb.scroll_offset, 2); // visible 3, bottom
        fb.nav_down();
        assert_eq!(fb.selection, 0);
        assert_eq!(fb.scroll_offset, 0);
        fb.nav_down();
        assert_eq!(fb.selection, 1);
        fb.nav_down();
        assert_eq!(fb.selection, 2);
        fb.nav_down();
        assert_eq!(fb.selection, 3);
        assert_eq!(fb.scroll_offset, 1);
    }

    #[test]
    fn page_up_down() {
        let mut fb = browser_with_items(vec!["..", "a", "b", "c", "d", "e", "f"]);
        fb.visible_items = 3;
        fb.selection = 5;
        fb.ensure_visible();
        assert_eq!(fb.scroll_offset, 3);
        fb.page_up();
        assert_eq!(fb.selection, 2);
        fb.page_down();
        assert_eq!(fb.selection, 5);
    }

    #[test]
    fn enter_child_and_parent() {
        let mut fb = FolderBrowser::new(BrowserMode::Platform("NES".into()), PathBuf::from("/roms"));
        fb.set_items(vec!["..".into(), "nes".into(), "snes".into()]);
        fb.selection = 1; // nes
        let next = fb.enter();
        assert_eq!(next, PathBuf::from("/roms/nes"));
        assert_eq!(fb.selection, 0);
        // geri ..
        fb.set_items(vec!["..".into(), "a".into()]);
        fb.selection = 0;
        let up = fb.enter();
        assert_eq!(up, PathBuf::from("/roms"));
    }

    #[test]
    fn enter_drive_windows() {
        let mut fb = FolderBrowser::new(BrowserMode::RomsRoot, PathBuf::from(""));
        fb.set_items(vec!["C:".into(), "D:".into()]);
        fb.selection = 0;
        let next = fb.enter();
        assert_eq!(next, PathBuf::from("C:\\"));
    }

    #[test]
    fn go_parent_resets() {
        let mut fb = FolderBrowser::new(BrowserMode::HistoryMove, PathBuf::from("/a/b"));
        fb.set_items(vec!["..".into(), "x".into()]);
        fb.selection = 1;
        let up = fb.go_parent();
        assert_eq!(up, PathBuf::from("/a"));
        assert_eq!(fb.selection, 0);
    }

    #[test]
    fn visible_slice() {
        let mut fb = browser_with_items(vec!["..", "a", "b", "c", "d"]);
        fb.visible_items = 2;
        fb.selection = 0;
        assert_eq!(fb.visible_slice(), &["..", "a"]);
        fb.selection = 3;
        fb.ensure_visible();
        assert_eq!(fb.visible_slice(), &["b", "c"]);
    }

    #[test]
    fn clamp_selection_on_set_items() {
        let mut fb = FolderBrowser::new(BrowserMode::RomsRoot, "");
        fb.selection = 5;
        fb.set_items(vec!["a".into(), "b".into()]);
        assert_eq!(fb.selection, 1);
        fb.set_items(vec![]);
        assert_eq!(fb.selection, 0);
    }

    #[test]
    fn mode_title() {
        assert_eq!(BrowserMode::RomsRoot.title(), "Select default ROMs folder");
        assert_eq!(BrowserMode::HistoryMove.title(), "Select destination folder");
        assert!(BrowserMode::Platform("NES".into()).title().contains("NES"));
    }
}
