//! Faz 12b — TVUI native shell.
//!
//! Strateji (ROADMAP_FAZ12): pygame TVUI'yi Rust'a port etmek yerine, mevcut WebUI
//! SPA'sını (`?mode=tv` ile 10-foot layout + gamepad/kumanda nav) native bir
//! pencerede gösteririz. Böylece tek bir frontend bakımı kalır.
//!
//! Varsayılan yol: harici kiosk tarayıcı (chromium/chrome)
//! `http://127.0.0.1:<port>/?mode=tv` ile tam ekran açılır (webkit2gtk gerektirmez).
//!
//! Gelecek: `wry`+`tao` ile bağımlılıksız webview penceresi (ayrı `webview` feature)
//! bu ortamdaki gdk-3 link çakışması nedeniyle şimdilik devre dışı; uygun makinede
//! eklenecek.

/// TASK-005-B — native SDL2/gilrs gamepad girdi yolu (yalnız `native-input`
/// feature ile derlenir; varsayılan build'de boş modül).
pub mod native_input;

/// TVUI URL'i — WebUI SPA'sı, TV modu etkin.
pub fn tv_url(port: u16) -> String {
    format!("http://127.0.0.1:{port}/?mode=tv")
}

/// Harici kiosk tarayıcı bulup tam ekran açar. Tarayıcı bulunamazsa `Err`.
pub fn launch(port: u16) -> Result<(), String> {
    let url = tv_url(port);
    for exe in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "chrome",
        "chrome.exe",
    ] {
        // Çalıştırılabilir mevcut mu?
        if std::process::Command::new(exe)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            let mut cmd = std::process::Command::new(exe);
            cmd.args([
                "--kiosk",
                "--start-fullscreen",
                "--app",
                &url,
                "--noerrdialogs",
                "--disable-translate",
                "--disable-infobars",
            ]);
            match cmd.spawn() {
                Ok(_) => return Ok(()),
                Err(e) => return Err(format!("{exe} başlatılamadı: {e}")),
            }
        }
    }
    Err("kiosk tarayıcı bulunamadı (chromium/chrome). Yüklü bir tarayıcı gerekli.".into())
}
