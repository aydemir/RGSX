//! Sistem tepsisi ikonu + bağlam menüsü (tray-icon + muda).
//!
//! Windows'ta tray-icon gizli bir pencerenin WndProc'u ile çalışır; bu pencereye
//! mesajların ulaşması için **aynı thread'de bir mesaj pump döngüsü** (GetMessageW)
//! çalıştırılmalıdır. Bu modül tray'i kendi `std::thread`'inde kurar ve o thread'de
//! pump + menu event'lerini işler; tarayıcı/Explorer eylemleri manager-bin'e kanal
//! ile iletilir. Auto-start toggle'ı thread içinde (registry + checkbox) ele alınır.
//!
//! Python karşılığı: `rgsx_manager.py::_setup_tray` (pystray menu).

use std::path::Path;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;

use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, GetMessageW, MSG};

use crate::autostart;

/// Tepsiden gelen eylemler — manager-bin bunları tokio'da işler.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    /// Tarayıcıda `http://localhost:{port}` açar.
    OpenUi,
    /// Tarayıcıda `http://localhost:{port}/settings` açar.
    OpenSettings,
    /// İndirme (ROMS) klasörünü Explorer'da açar.
    OpenDownloads,
    /// Log klasörünü Explorer'da açar.
    OpenLogs,
    /// Çıkış.
    Quit,
}

/// Tray yapılandırması.
pub struct TrayConfig {
    pub icon_path: String,
    /// Auto-start menü girişinin ilk işaretli durumu.
    pub autostart_checked: bool,
}

const ID_OPEN_UI: &str = "open_ui";
const ID_SETTINGS: &str = "settings";
const ID_AUTOSTART: &str = "autostart";
const ID_DOWNLOADS: &str = "downloads";
const ID_LOGS: &str = "logs";
const ID_QUIT: &str = "quit";

/// Aktif tray ikonu + onu besleyen thread'i elinde tutar.
pub struct Tray {
    actions: mpsc::Receiver<TrayAction>,
}

impl Tray {
    /// Tray'i başlatır; `actions` kanalından olayları oku.
    pub fn start(cfg: TrayConfig) -> std::io::Result<Tray> {
        let (tx, rx) = mpsc::channel::<TrayAction>();

        // Tray'i kendi thread'inde kur (pump da bu thread'de olmalı).
        std::thread::Builder::new()
            .name("rgsx-tray".into())
            .spawn(move || {
                let icon = load_icon(&cfg.icon_path);
                let (menu, autostart_item) = build_menu(cfg.autostart_checked);

                let _tray = match TrayIconBuilder::new()
                    .with_tooltip("RGSX Download Manager")
                    .with_icon(icon)
                    .with_menu(Box::new(menu))
                    .build()
                {
                    Ok(t) => Some(t),
                    Err(e) => {
                        eprintln!("[tray] ikon oluşturulamadı: {e}");
                        None
                    }
                };

                // Windows mesaj pump'ı — GetMessageW bloklayıcıdır; tray pencere
                // mesajlarını işler ve her mesajdan sonra menu event'lerini boşaltır.
                let mut msg: MSG = unsafe { std::mem::zeroed() };
                unsafe {
                    loop {
                        let ret = GetMessageW(&mut msg, HWND::default(), 0, 0);
                        // -1 = hata, 0 = WM_QUIT.
                        if ret.0 <= 0 {
                            break;
                        }
                        let _ = DispatchMessageW(&msg);
                        drain_menu_events(&tx, autostart_item.as_ref());
                    }
                }
            })?;

        Ok(Tray { actions: rx })
    }

    /// Bekleyen tray eylemini bloklamadan alır.
    pub fn try_action(&self) -> Result<TrayAction, TryRecvError> {
        self.actions.try_recv()
    }
}

/// ICO/PNG yüklemeyi dener; başarısızsa 1x1 saydam placeholder üretir.
fn load_icon(path: &str) -> Icon {
    if let Ok(icon) = Icon::from_path(Path::new(path), Some((32, 32))) {
        return icon;
    }
    // RGBA 1x1 saydam piksel — ikon yüklenemezse tepsisi görünmez bırakmak yerine boş.
    Icon::from_rgba(vec![0, 0, 0, 0], 1, 1).unwrap_or_else(|_| {
        // from_rgba neredeyse asla hata vermez; savunma olarak 1x1 opak siyah.
        Icon::from_rgba(vec![0, 0, 0, 255], 1, 1).expect("1x1 RGBA ikonu geçerli")
    })
}

fn build_menu(autostart_checked: bool) -> (Menu, Option<CheckMenuItem>) {
    let open_ui = MenuItem::with_id(ID_OPEN_UI, "Open Web UI", true, None);
    let settings = MenuItem::with_id(ID_SETTINGS, "Ayarlar", true, None);
    let autostart = CheckMenuItem::with_id(ID_AUTOSTART, "Auto-start on boot", true, autostart_checked, None);
    let downloads = MenuItem::with_id(ID_DOWNLOADS, "Downloads folder", true, None);
    let logs = MenuItem::with_id(ID_LOGS, "Logs folder", true, None);
    let quit = MenuItem::with_id(ID_QUIT, "Exit", true, None);

    let menu = Menu::new();
    let _ = menu.append(&open_ui);
    let _ = menu.append(&settings);
    let _ = menu.append(&autostart);
    let _ = menu.append(&downloads);
    let _ = menu.append(&logs);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&quit);

    (menu, Some(autostart))
}

fn drain_menu_events(tx: &mpsc::Sender<TrayAction>, autostart_item: Option<&CheckMenuItem>) {
    // MenuEvent::receiver() globaldir; o anki MenuEvent'leri boşalt.
    while let Ok(event) = MenuEvent::receiver().try_recv() {
        match event.id().as_ref() {
            ID_OPEN_UI => { let _ = tx.send(TrayAction::OpenUi); }
            ID_SETTINGS => { let _ = tx.send(TrayAction::OpenSettings); }
            ID_AUTOSTART => {
                // Thread içinde: registry toggle + checkbox güncelle (MenuItem burada yaşar).
                let enabled = autostart::is_enabled();
                let result = if enabled {
                    autostart::remove()
                } else {
                    autostart::install(&autostart::command_self())
                };
                if let Some(item) = autostart_item {
                    item.set_checked(!enabled);
                }
                match result {
                    Ok(()) => eprintln!("[tray] auto-start {}", if enabled { "kapatıldı" } else { "açıldı" }),
                    Err(e) => eprintln!("[tray] auto-start güncellenemedi: {e}"),
                }
            }
            ID_DOWNLOADS => { let _ = tx.send(TrayAction::OpenDownloads); }
            ID_LOGS => { let _ = tx.send(TrayAction::OpenLogs); }
            ID_QUIT => { let _ = tx.send(TrayAction::Quit); }
            _ => {}
        }
    }
}
