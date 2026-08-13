//! TVUI başlatıcı binary. `RGSX_TVUI_PORT` (veya arg) ile WebUI portunu alır.

fn main() {
    let port: u16 = std::env::var("RGSX_TVUI_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or_else(|| std::env::args().nth(1).and_then(|a| a.parse().ok()))
        .unwrap_or(5000);

    match manager_tvui::launch(port) {
        Ok(()) => {}
        Err(e) => {
            eprintln!("TVUI başlatılamadı: {e}");
            std::process::exit(1);
        }
    }
}
