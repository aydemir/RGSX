//! TASK-012g — SDL2 native shell (temel: tam ekran init + arka plan paleti çizimi).
//!
//! `tvui.py` + `display/*` pygame `draw_*`'ları TASK-012h ve sonrasında SDL2
//! primitives'e portlanır. Bu modül yalnızca shell'i kurar ve `theme.json`
//! paletiyle arka plan gradyanını çizer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::net::{
    apply_ui_action, expire_stale_restart_at, tvui_lock, ui_decision, SharedTvuiState, UiKey,
};
use crate::state::{MenuState, TvuiScreen, GRID_COLS, GRID_PER_PAGE, GRID_ROWS};
use crate::theme::Theme;

/// Box-art kök dizinini çözer (SDL'siz, test edilebilir). Sıra:
/// 1. `RGSX_IMAGES_FOLDER` (katalog `NativeCatalog` ile aynı kaynak)
/// 2. `RGSX_DATA_DIR/images`
/// 3. exe'den RetroBat anchor: `roms/ports/RGSX` yanındaki exe → 3×parent =
///    RetroBat root → `saves/ports/rgsx/images`
/// 4. tema `icons.path` (CWD-relative, dev fallback)
/// Bulunamazsa tema yolunu aynen döner (çağıran fallback kutuyu korur).
pub fn resolve_icons_path(theme_icons_path: &str) -> String {
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(p) = std::env::var("RGSX_IMAGES_FOLDER") {
        if !p.trim().is_empty() {
            cands.push(std::path::PathBuf::from(p));
        }
    }
    if let Ok(d) = std::env::var("RGSX_DATA_DIR") {
        if !d.trim().is_empty() {
            cands.push(std::path::PathBuf::from(d).join("images"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let root = dir
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent());
            if let Some(r) = root {
                cands.push(r.join("saves").join("ports").join("rgsx").join("images"));
            }
        }
    }
    cands.push(std::path::PathBuf::from(theme_icons_path));
    for c in &cands {
        if c.is_dir() {
            return c.to_string_lossy().into_owned();
        }
    }
    theme_icons_path.to_string()
}

/// Seçili tile pulse ölçeği (Python parity: `1.15 + 0.05*sin`, 600ms periyot).
/// `now_ms` monoton milisaniye (SystemTime/UNIX_EPOCH). Saf, test edilebilir.
pub fn selection_pulse_scale(now_ms: u64) -> f32 {
    const BASE: f32 = 1.15;
    const AMP: f32 = 0.05;
    const PERIOD_MS: f32 = 600.0;
    let phase = (now_ms as f32 % PERIOD_MS) / PERIOD_MS * std::f32::consts::TAU;
    BASE + AMP * phase.sin()
}

/// Monoton duvar saati (ms). SDL'siz; pulse fazı buradan beslenir.
pub fn wall_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Yuvarlak dikdörtgen satır aralıkları (SDL'siz, test edilebilir).
/// Dönüş: `(y, x0, x1)` — her satırda doldurulacak yatay aralık.
/// `r` yarıçapı min(w,h)/2'ye clamp'lenir.
pub fn rounded_fill_spans(w: u32, h: u32, r: u32) -> Vec<(u32, u32, u32)> {
    if w == 0 || h == 0 {
        return Vec::new();
    }
    let r = r.min(w / 2).min(h / 2);
    let mut out = Vec::with_capacity(h as usize);
    for y in 0..h {
        // Köşe dairelerinin yatay girintisi (üst/alt şeritlerde).
        let dy = if y < r {
            r - y
        } else if y >= h - r {
            y - (h - r) + 1
        } else {
            0
        };
        let inset = if dy == 0 {
            0
        } else {
            // x = r - sqrt(r² - dy²), yukarı yuvarla.
            let v = (r * r).saturating_sub(dy * dy);
            let root = (v as f64).sqrt().floor() as u32;
            r.saturating_sub(root)
        };
        out.push((y, inset, w.saturating_sub(inset)));
    }
    out
}

/// İçi dolu yuvarlak dikdörtgen (Python `border_radius` parity).
fn fill_rounded_rect(
    canvas: &mut Canvas<Window>,
    color: Color,
    rect: sdl2::rect::Rect,
    radius: u32,
) {
    if rect.width() == 0 || rect.height() == 0 {
        return;
    }
    canvas.set_draw_color(color);
    for (y, x0, x1) in rounded_fill_spans(rect.width(), rect.height(), radius) {
        if x1 > x0 {
            let _ = canvas.draw_line(
                (rect.x() + x0 as i32, rect.y() + y as i32),
                (rect.x() + x1 as i32 - 1, rect.y() + y as i32),
            );
        }
    }
}

/// Yuvarlak dikdörtgen çerçeve (dış çizgi; köşeler daire yayıyla).
fn draw_rounded_rect(
    canvas: &mut Canvas<Window>,
    color: Color,
    rect: sdl2::rect::Rect,
    radius: u32,
) {
    if rect.width() == 0 || rect.height() == 0 {
        return;
    }
    let r = radius.min(rect.width() / 2).min(rect.height() / 2);
    canvas.set_draw_color(color);
    let (x, y, w, h) = (rect.x(), rect.y(), rect.width() as i32, rect.height() as i32);
    // Düz kenarlar (köşe yayları hariç).
    let _ = canvas.draw_line((x + r as i32, y), (x + w - r as i32, y));
    let _ = canvas.draw_line((x + r as i32, y + h - 1), (x + w - r as i32, y + h - 1));
    let _ = canvas.draw_line((x, y + r as i32), (x, y + h - r as i32));
    let _ = canvas.draw_line((x + w - 1, y + r as i32), (x + w - 1, y + h - r as i32));
    // 4 köşe yayı: her köşede YALNIZCA o köşeye bakan çeyrek çizilir
    // (tam daire çizilirse köşe dışına taşan halka görünür).
    // Köşe merkezleri + içe bakan işaretler: TL(-,-), TR(+,-), BL(-,+), BR(+,+).
    let corners = [
        (x + r as i32, y + r as i32, -1, -1),
        (x + w - r as i32 - 1, y + r as i32, 1, -1),
        (x + r as i32, y + h - r as i32 - 1, -1, 1),
        (x + w - r as i32 - 1, y + h - r as i32 - 1, 1, 1),
    ];
    for (cx, cy, qx, qy) in corners {
        for dy in 0..=r as i32 {
            let dx = ((r as f64 * r as f64 - dy as f64 * dy as f64).max(0.0).sqrt()).round() as i32;
            // Çeyrek: dy'yi işaretle, dx'i işaretle (daire simetrisi).
            let _ = canvas.draw_point((cx + qx * dx, cy + qy * dy));
            let _ = canvas.draw_point((cx + qx * dy, cy + qy * dx));
        }
    }
}

/// Platform kaynak rozet anahtarı (Python `get_platform_source_badge_key` parity):
/// ismin sonundaki `(kaynak)` → Archive/LolRoms/Torrent/1Fichier/Vimms/EdgeEmu.
pub fn source_badge_key(platform_name: &str) -> Option<&'static str> {
    let text = platform_name.trim();
    if text.is_empty() {
        return None;
    }
    // Son `(...)` grubunu al.
    let open = text.rfind('(')?;
    let close = text.rfind(')')?;
    if close < open || close != text.len() - 1 {
        return None;
    }
    match text[open + 1..close].trim().to_ascii_lowercase().as_str() {
        "archive" => Some("Archive"),
        "lolroms" => Some("LolRoms"),
        "torrent" => Some("Torrent"),
        "1fichier" => Some("1Fichier"),
        "vimms" => Some("Vimms"),
        "edgeemu" | "edgeemu.net" => Some("EdgeEmu"),
        _ => None,
    }
}

/// Rozet stili: (etiket, çerçeve rengi, metin rengi) — Python `style_map` parity.
pub fn source_badge_style(key: &str) -> Option<(&'static str, (u8, u8, u8, u8), (u8, u8, u8, u8))> {
    match key {
        "Archive" => Some(("AR", (48, 48, 48, 235), (35, 35, 35, 255))),
        "LolRoms" => Some(("LOL", (0, 255, 255, 230), (61, 19, 110, 255))),
        "Vimms" => Some(("VL", (208, 208, 208, 235), (24, 77, 176, 255))),
        "Torrent" => Some(("TOR", (97, 164, 64, 235), (37, 90, 24, 255))),
        "1Fichier" => Some(("1F", (208, 208, 208, 235), (24, 77, 176, 255))),
        "EdgeEmu" => Some(("EMU", (41, 126, 196, 235), (24, 77, 176, 255))),
        _ => None,
    }
}

/// Tile sağ-üst kaynak rozeti (beyaz yuvarlak kutu + kısa etiket).
/// Boyut: clamp(20, min(w,h)*0.24, 44), içten payda sağ üstte.
fn draw_source_badge(
    canvas: &mut Canvas<Window>,
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
    tile: sdl2::rect::Rect,
    platform_name: &str,
) {
    let key = match source_badge_key(platform_name) {
        Some(k) => k,
        None => return,
    };
    let (label, border, text_c) = match source_badge_style(key) {
        Some(s) => s,
        None => return,
    };
    let size = ((tile.width().min(tile.height()) as f32 * 0.24) as u32).clamp(20, 44);
    let inset = (5u32).max(size / 6);
    let r = sdl2::rect::Rect::new(
        tile.x() + tile.width() as i32 - size as i32 - inset as i32,
        tile.y() + inset as i32,
        size,
        size,
    );
    fill_rounded_rect(canvas, Color::RGBA(255, 255, 255, 242), r, (size / 4).max(8));
    draw_rounded_rect(canvas, to_color(border), r, (size / 4).max(8));
    let _ = crate::text::draw_text_centered(canvas, tc, label, text_c, r, 9, font_scale);
}

/// Platform logosunu blit eder; dosyası yoksa `default.png` fallback'ini dener
/// (Python `miss → default.png` parity; ikisi de yoksa false → kutu kalır).
fn blit_platform_art<'a>(
    art: &mut crate::boxart::SdlBoxArtCache<'a>,
    canvas: &mut Canvas<Window>,
    tc: &'a TextureCreator<WindowContext>,
    icons_path: &str,
    icon_path: &str,
    dst: sdl2::rect::Rect,
) -> bool {
    if art.blit(canvas, tc, icon_path, dst) {
        return true;
    }
    let fallback = format!("{}/default.png", icons_path.trim_end_matches('/'));
    if fallback != icon_path {
        return art.blit(canvas, tc, &fallback, dst);
    }
    false
}

/// Sol rozet bloğu (3 satır) dahil toplam header yüksekliği: 8 + 3*22 + 12.
/// Grid `margin_top` buradan türetilir.
pub const HEADER_BLOCK_H: u32 = 86;

/// Python `grid._format_disk_size_gb` parity: "241 GB" (boşluklu).
pub fn format_disk_gb(size_bytes: u64) -> String {
    let gb = size_bytes as f64 / (1024.0 * 1024.0 * 1024.0);
    if gb >= 100.0 {
        format!("{gb:.0} GB")
    } else if gb >= 10.0 {
        format!("{gb:.1} GB")
    } else {
        format!("{gb:.2} GB")
    }
}

/// Gömülü `version.json` → "2.6.5.6" (derleme anında, deploy'da da geçerli).
pub fn app_version() -> String {
    const RAW: &str = include_str!("../../../version.json");
    serde_json::from_str::<serde_json::Value>(RAW)
        .ok()
        .and_then(|v| v.get("version").and_then(|x| x.as_str()).map(|s| s.to_string()))
        .unwrap_or_else(|| "?.?.?".to_string())
}

/// LAN IP (manager-bin `local_lan_ip` parity, std-only UDP numarası, trafik yok).
/// Süreç boyu cache (IP değişimi yeniden başlatmayla alınır).
pub fn lan_ip() -> String {
    static CACHE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            (|| {
                let sock = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
                sock.connect("8.8.8.8:80").ok()?;
                sock.local_addr().ok().map(|a| a.ip().to_string())
            })()
            .unwrap_or_else(|| "127.0.0.1".to_string())
        })
        .clone()
}

/// Manager portu: `RGSX_MANAGER_BIN_PORT` > `RGSX_TVUI_PORT` > 5000.
pub fn manager_port() -> u16 {
    std::env::var("RGSX_MANAGER_BIN_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or_else(|| {
            std::env::var("RGSX_TVUI_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
        })
        .unwrap_or(5000)
}

/// ROM klasörü disk satırı (Python `get_default_disk_space_line` parity):
/// `"[HDD] 241 GB/446 GB (54% free)"`. Yol yoksa/disk bulunamazsa `""`.
/// sysinfo taraması pahalı → yol başına 10 sn TTL cache (kare döngüsü için).
pub fn disk_line(roms_path: &str) -> String {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<(std::time::Instant, String, String)>> =
        std::sync::OnceLock::new();
    let now = std::time::Instant::now();
    let slot = CACHE.get_or_init(|| {
        std::sync::Mutex::new((now, String::new(), String::new()))
    });
    if let Ok(guard) = slot.lock() {
        if guard.1 == roms_path && now.duration_since(guard.0).as_secs() < 10 {
            return guard.2.clone();
        }
    }
    let fresh = disk_line_uncached(roms_path);
    if let Ok(mut guard) = slot.lock() {
        *guard = (now, roms_path.to_string(), fresh.clone());
    }
    fresh
}

/// `disk_line` saf çekirdeği (testler burayı kullanır).
fn disk_line_uncached(roms_path: &str) -> String {
    if roms_path.trim().is_empty() {
        return String::new();
    }
    let path = std::path::Path::new(roms_path);
    let disks = sysinfo::Disks::new_with_refreshed_list();
    // En spesifik mount point (en uzun prefix).
    let best = disks
        .iter()
        .filter(|d| path.starts_with(d.mount_point()))
        .max_by_key(|d| d.mount_point().as_os_str().len());
    match best {
        Some(d) => {
            let total = d.total_space();
            let free = d.available_space();
            let pct = if total > 0 {
                ((free as f64 / total as f64) * 100.0).round() as u64
            } else {
                0
            };
            let free_word = crate::i18n::tcached("disk_percent_free");
            format!(
                "[HDD] {}/{} ({}% {})",
                format_disk_gb(free),
                format_disk_gb(total),
                pct,
                free_word
            )
        }
        None => String::new(),
    }
}

/// Header rozet metinleri (SDL'siz, test edilebilir; Python `draw_grid` header parity):
/// sol satırlar `[Page p/t, disk, Res : WxH]`, orta `{ad}  ({n})`,
/// sağ satırlar `[vVer, ip]`. Dil `t()` ile (upstream default EN).
pub fn header_data(
    platform_name: &str,
    games_count: usize,
    page_text: Option<&str>,
    games_word: Option<&str>,
    w: u32,
    h: u32,
    disk: &str,
    version: &str,
    ip: &str,
) -> (Vec<String>, String, Vec<String>) {
    let mut left = Vec::new();
    if let Some(p) = page_text {
        if !p.is_empty() {
            left.push(p.to_string());
        }
    }
    if !disk.is_empty() {
        left.push(disk.to_string());
    }
    left.push(format!("Res : {w}x{h}"));
    let name = if platform_name.trim().is_empty() {
        "RGSX"
    } else {
        platform_name.trim()
    };
    let middle = match games_word {
        Some(word) => format!("{name} ({games_count} {word})"),
        None => format!("{name}  ({games_count})"),
    };
    let right = vec![format!("v{version}"), ip.to_string()];
    (left, middle, right)
}

/// `platform_page` yer tutuculu format (`Page {0}/{1}`), o anki dilden beslenir.
fn format_page(page: usize, total_pages: usize) -> String {
    format_page_with(&crate::i18n::tcached("platform_page"), page, total_pages)
}

/// Saf çekirdek (testler yerelden bağımsız burayı kullanır).
fn format_page_with(pat: &str, page: usize, total_pages: usize) -> String {
    if pat.contains("{0}") {
        pat.replacen("{0}", &(page + 1).to_string(), 1)
            .replacen("{1}", &total_pages.to_string(), 1)
    } else {
        format!("Page {}/{}", page + 1, total_pages)
    }
}
/// Header çubuğu: sol/orta/sağ rozet (Python `grid.py` header parity).
/// Orta rozet seçili platformu gösterir; `selected` yoksa "RGSX".
fn draw_header(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let (name, count, games_word) = if matches!(screen.menu, MenuState::GameList) {
        // Oyun listesi: orta rozet `{platform} ({n} games)` (Python parity).
        let n = screen.platforms.get(screen.selected_platform);
        (
            n.map(|p| p.name.as_str()).unwrap_or("RGSX"),
            screen.games.len(),
            Some(crate::i18n::tcached("games")),
        )
    } else {
        let (nm, cnt) = screen
            .platforms
            .get(screen.selected_platform)
            .map(|p| (p.name.as_str(), p.games_count))
            .unwrap_or(("RGSX", 0));
        (nm, cnt, None)
    };
    let total_pages = (screen.platforms.len() + GRID_PER_PAGE - 1) / GRID_PER_PAGE;
    let page = if screen.platforms.is_empty() {
        0
    } else {
        screen.selected_platform.min(screen.platforms.len() - 1) / GRID_PER_PAGE
    };
    let roms = std::env::var("RGSX_ROMS_FOLDER").unwrap_or_default();
    let page_text = if total_pages > 1 && matches!(screen.menu, MenuState::PlatformGrid) {
        Some(format_page(page, total_pages))
    } else {
        None
    };
    let (left, middle, right) = header_data(
        name,
        count,
        page_text.as_deref(),
        games_word.as_deref(),
        w,
        h,
        &disk_line(&roms),
        &app_version(),
        &lan_ip(),
    );
    let y = 8i32;
    let bw_left = (w * 24 / 100).clamp(200, 380);
    let bw_mid = (w * 30 / 100).clamp(280, 470);
    let bw_right = (w * 17 / 100).clamp(170, 280);
    draw_badge_lines(canvas, theme, tc, font_scale, 20, y, bw_left, &left, 13);
    let mid_h = draw_badge_lines(
        canvas,
        theme,
        tc,
        font_scale,
        (w as i32 - bw_mid as i32) / 2,
        y,
        bw_mid,
        std::slice::from_ref(&middle),
        18,
    );
    let _ = mid_h;
    draw_badge_lines(
        canvas,
        theme,
        tc,
        font_scale,
        w as i32 - bw_right as i32 - 20,
        y,
        bw_right,
        &right,
        13,
    );
}

/// Çok satırlı rozet kutusu (Python `draw_header_badge` parity): her satır ortalı,
/// metin `text` rengi (beyaz). Dönüş: kutu yüksekliği.
fn draw_badge_lines(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
    x: i32,
    y: i32,
    w: u32,
    lines: &[String],
    base_size: u16,
) -> u32 {
    if lines.is_empty() || w == 0 || x < 0 {
        return 0;
    }
    let line_h = ((20.0 * font_scale) as u32).max(14);
    let h = lines.len() as u32 * line_h + 12;
    let r = sdl2::rect::Rect::new(x, y, w, h);
    fill_rounded_rect(canvas, to_color(theme.color("button_idle")), r, 10);
    draw_rounded_rect(canvas, to_color(theme.color("border")), r, 10);
    for (i, line) in lines.iter().enumerate() {
        let rr = sdl2::rect::Rect::new(
            x + 6,
            y + 6 + i as i32 * line_h as i32,
            w.saturating_sub(12),
            line_h,
        );
        let _ = crate::text::draw_text_centered(canvas, tc, line, theme.color("text"), rr, base_size, font_scale);
    }
    h
}

fn to_color((r, g, b, a): (u8, u8, u8, u8)) -> Color {
    Color::RGBA(r, g, b, a)
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

fn a11y_bg(theme: &Theme, screen: &TvuiScreen, preset: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
    screen.a11y.effective_background(theme, preset)
}

/// Seçili arka plan preset'ini dikey gradyan olarak çizer (top → bottom).
/// Faz C (bulgu 12): gradyan her frame'de h adet `draw_line` yerine BİR KEZ
/// texture'a üretilir ve blit edilir; pencere boyutu değişirse yenilenir.
/// Texture üretilemezse eski scanline yoluna düşer (doğruluk > zarafet).
fn draw_background<'a>(
    canvas: &mut Canvas<Window>,
    tc: &'a TextureCreator<WindowContext>,
    cache: &mut Option<(u32, u32, Texture<'a>)>,
    theme: &Theme,
    screen: &TvuiScreen,
    preset: &str,
) -> (u32, u32) {
    let (w, h) = match canvas.output_size() {
        Ok((w, h)) if w > 0 && h > 0 => (w, h),
        _ => (1280, 720),
    };
    let (top, bottom) = a11y_bg(theme, screen, preset);
    let stale = !matches!(cache, Some((cw, ch, _)) if *cw == w && *ch == h);
    if stale {
        // Tam w*h gradyan (tek seferlik ~3.7MB, cache'te yaşar).
        let mut pixels: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let t = if h <= 1 {
                0.0
            } else {
                y as f32 / (h - 1) as f32
            };
            let (r, g, b) = (
                lerp(top.0, bottom.0, t),
                lerp(top.1, bottom.1, t),
                lerp(top.2, bottom.2, t),
            );
            for _ in 0..w {
                pixels.extend_from_slice(&[r, g, b, 255]);
            }
        }
        match tc.create_texture_static(PixelFormatEnum::RGBA32, w, h) {
            Ok(mut tex) => {
                if tex.update(None, &pixels, (w * 4) as usize).is_ok() {
                    *cache = Some((w, h, tex));
                } else {
                    eprintln!("TVUI arka plan texture güncellenemedi");
                }
            }
            Err(e) => eprintln!("TVUI arka plan texture üretilemedi: {e}"),
        }
    }
    match cache {
        Some((_, _, tex)) => {
            let _ = canvas.copy(tex, None, None);
        }
        None => {
            // Fallback: satır satır gradyan (eski davranış).
            for y in 0..h {
                let t = if h <= 1 {
                    0.0
                } else {
                    y as f32 / (h - 1) as f32
                };
                canvas.set_draw_color(Color::RGB(
                    lerp(top.0, bottom.0, t),
                    lerp(top.1, bottom.1, t),
                    lerp(top.2, bottom.2, t),
                ));
                let _ = canvas.draw_line((0, y as i32), (w as i32, y as i32));
            }
        }
    }
    (w, h)
}

/// Loading metinleri — SDL'siz, test edilebilir.
/// `(başlık, alt_satır)`: başlık stage'i gösterir (boşsa "Yükleniyor"),
/// alt satır yüzde + stage.
pub fn loading_texts(pct_0_100: i64, stage: &str, error: Option<&str>) -> (String, String) {
    if let Some(err) = error {
        let short: String = err.chars().take(60).collect();
        return (
            "Hata".to_string(),
            format!("{short} ({}%)", pct_0_100.clamp(0, 100)),
        );
    }
    let title = if stage.trim().is_empty() {
        "Yükleniyor".to_string()
    } else {
        stage.chars().take(48).collect()
    };
    (title, format!("{}%", pct_0_100.clamp(0, 100)))
}

/// Loading panel geometrisi — SDL'siz (bar + panel rect).
/// Dönüş: `(panel_x, panel_y, panel_w, panel_h, bar_x, bar_y, bar_w, bar_h)`.
pub fn loading_layout(w: u32, h: u32) -> (i32, i32, u32, u32, i32, i32, u32, u32) {
    let bar_w = ((w as i32) * 60 / 100).max(40) as u32;
    let bar_h: u32 = 24;
    let x = ((w as i32 - bar_w as i32) / 2).max(0);
    let y = (h as i32 / 2).max(0);
    let pad: i32 = 20;
    let px = (x - pad).max(0);
    let py = (y - 52).max(0);
    let pw = bar_w + (pad * 2) as u32;
    let ph: u32 = 110;
    (px, py, pw, ph, x, y, bar_w, bar_h)
}

/// Açılış loading bar'ı: SSE `catalog_update` ilerlemesini `state`'ten okur.
/// `ready` oluncaya kadar ortada yuvarlak panel + bar çizer; stage başlıkta,
/// yüzde alt satırda. Hata varsa kırmızı panel + hata metni.
fn draw_loading(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let (pct_raw, stage, error) = {
        let s = tvui_lock(state);
        (s.pct.clamp(0, 100), s.stage.clone(), s.error.clone())
    };
    let pct = pct_raw as f32 / 100.0;
    let (px, py, pw, ph, x, y, bar_w, bar_h) = loading_layout(w, h);
    let panel = sdl2::rect::Rect::new(px, py, pw, ph);
    let is_err = error.is_some();
    let border_key = if is_err { "error_text" } else { "border" };
    fill_rounded_rect(canvas, to_color(theme.color("button_idle")), panel, 12);
    draw_rounded_rect(canvas, to_color(theme.color(border_key)), panel, 12);
    if is_err {
        canvas.set_draw_color(to_color(theme.color("error_text")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, (y - 40).max(0), w, 6));
    }
    let (title, sub) = loading_texts(pct_raw, &stage, error.as_deref());
    let title_c = if is_err {
        theme.color("error_text")
    } else {
        theme.color("text")
    };
    let _ = crate::text::draw_text_centered(
        canvas,
        tc,
        &title,
        title_c,
        sdl2::rect::Rect::new(px, py + 10, pw, 26),
        14,
        font_scale,
    );
    let bar_rect = sdl2::rect::Rect::new(x, y, bar_w, bar_h);
    draw_rounded_rect(canvas, to_color(theme.color("border")), bar_rect, 8);
    let fill_w = (bar_w as f32 * pct) as u32;
    if fill_w > 0 {
        let inner = sdl2::rect::Rect::new(x + 2, y + 2, fill_w.saturating_sub(4).min(bar_w.saturating_sub(4)), bar_h.saturating_sub(4));
        if inner.width() > 0 && inner.height() > 0 {
            fill_rounded_rect(
                canvas,
                to_color(theme.color(if is_err { "error_text" } else { "neon" })),
                inner,
                6,
            );
        }
    }
    let _ = crate::text::draw_text_centered(
        canvas,
        tc,
        &sub,
        if is_err {
            theme.color("error_text")
        } else {
            theme.color("neon")
        },
        sdl2::rect::Rect::new(x, y + bar_h as i32 + 8, bar_w, 24),
        13,
        font_scale,
    );
}

/// `ready` sonrası platform grid'i: `/api/platforms`'tan gelen `state.platforms`
/// listesini tile olarak dizer. Faz 3: seçili tile `border_selected` + scale ile vurgulanır.
fn draw_grid<'a>(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &'a TextureCreator<WindowContext>,
    font_scale: f32,
    art: &mut crate::boxart::SdlBoxArtCache<'a>,
) {
    let (platforms, offline) = {
        let s = tvui_lock(state);
        (s.platforms.clone(), s.offline)
    };
    if offline {
        canvas.set_draw_color(to_color(theme.color("error_text")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, 6));
        let _ = crate::text::draw_text(canvas, tc, "CEVRIMDISI", theme.color("error_text"), 10, 10, 12, font_scale);

    }
    if platforms.is_empty() {
        let _ = crate::text::draw_text_centered(canvas, tc, "platform yok", theme.color("neon"), sdl2::rect::Rect::new(0, 100, w, 40), 14, font_scale);

        return;
    }
    // 3×4 sayfa geometrisi (Python `display/grid.py` parity):
    // margin_lr = 0.026W, margin_top = max(0.140H, header+clearance),
    // margin_bottom = footer rezervi; hücre = min(col_w, row_h), gap = 0.15*hücre.
    let cols = GRID_COLS as u32;
    let rows = GRID_ROWS as u32;
    let per = GRID_PER_PAGE;
    let n = platforms.len();
    let page = (screen.selected_platform.min(n - 1)) / per;
    let start = page * per;
    let end = (start + per).min(n);
    let margin_lr = ((w as f32 * 0.026) as u32).max(12);
    let header_bottom = 8 + HEADER_BLOCK_H;
    let clearance = ((h as f32 * 0.03) as u32).max(20);
    let margin_top = ((h as f32 * 0.14) as u32).max(header_bottom + clearance);
    let footer_gap = ((h as f32 * 0.018) as u32).max(12);
    let margin_bottom = (70 + footer_gap).max(((h as f32 * 0.118) as u32).max(70));
    let avail_w = w.saturating_sub(margin_lr * 2);
    let avail_h = h.saturating_sub(margin_top + margin_bottom);
    let col_w = (avail_w / cols).max(1);
    let row_h = (avail_h / rows).max(1);
    let cell = col_w.min(row_h);
    let gap = ((cell as f32 * 0.15) as u32).max(4);
    let tile_w = col_w.saturating_sub(gap);
    let tile_h = row_h.saturating_sub(gap);
    let sel = if matches!(screen.menu, MenuState::PlatformGrid) {
        Some(screen.selected_platform)
    } else {
        None
    };
    let trans_scale = screen
        .transition
        .as_ref()
        .and_then(|tr| tr.sample(Instant::now()))
        .map(|(s, _)| s)
        .unwrap_or(1.0);
    for (local_i, p) in platforms[start..end].iter().enumerate() {
        let abs_i = start + local_i;
        let col = (local_i as u32) % cols;
        let row = (local_i as u32) / cols;
        let base_x = margin_lr + col * col_w + gap / 2;
        let base_y = margin_top + row * row_h + gap / 2;
        let is_sel = sel == Some(abs_i);
        // Box-art cache: ikon yolunu çöz (folder bazlı, platform_image fallback)
        let icon_path = crate::render::BoxArtCache::icon_path_for(&p.folder, &theme.icons.path);
        // Seçili tile: transition scale + border_selected
        if is_sel {
            // Seçili vurgu: transition varsa onun ölçeği, yoksa pulse (1.15±0.05).
            let scale = if screen.transition.is_some() {
                trans_scale
            } else {
                selection_pulse_scale(wall_ms())
            };
            let sw = (tile_w as f32 * scale) as u32;
            let sh = (tile_h as f32 * scale) as u32;
            let dx = ((tile_w as i32 - sw as i32) / 2) as i32;
            let dy = ((tile_h as i32 - sh as i32) / 2) as i32;
            let x = base_x as i32 + dx;
            let y = base_y as i32 + dy;
            // Neon glow: dışta yuvarlak katman.
            let glow_pad = 6i32;
            draw_rounded_rect(
                canvas,
                to_color(theme.color("neon")),
                sdl2::rect::Rect::new(
                    x - glow_pad,
                    y - glow_pad,
                    sw + (glow_pad * 2) as u32,
                    sh + (glow_pad * 2) as u32,
                ),
                14,
            );
            let pad = 2i32;
            fill_rounded_rect(
                canvas,
                to_color(theme.color("button_selected")),
                sdl2::rect::Rect::new(
                    x - pad,
                    y - pad,
                    sw + (pad * 2) as u32,
                    sh + (pad * 2) as u32,
                ),
                12,
            );
            draw_rounded_rect(
                canvas,
                to_color(theme.color("text")),
                sdl2::rect::Rect::new(
                    x - pad,
                    y - pad,
                    sw + (pad * 2) as u32,
                    sh + (pad * 2) as u32,
                ),
                12,
            );
            // Box-art: secili tile ile birlikte olceklenen rect'e blit (yoksa default.png, o da yoksa kutu kalir).
            // Upstream sözleşmesi: tile'da isim YAZISI YOK (logo + source badge).
            let tile_rect = sdl2::rect::Rect::new(x, y, sw, sh);
            let _ = blit_platform_art(art, canvas, tc, &theme.icons.path, &icon_path, tile_rect);
            draw_source_badge(canvas, tc, font_scale, tile_rect, &p.name);

        } else {
            let tile_rect = sdl2::rect::Rect::new(base_x as i32, base_y as i32, tile_w, tile_h);
            fill_rounded_rect(canvas, to_color(theme.color("button_idle")), tile_rect, 12);
            draw_rounded_rect(canvas, to_color(theme.color("neon")), tile_rect, 12);
            let _ = blit_platform_art(art, canvas, tc, &theme.icons.path, &icon_path, tile_rect);
            draw_source_badge(canvas, tc, font_scale, tile_rect, &p.name);

        }
    }
}

/// Faz 4: oyun listesi — tablo (Python `draw_game_list` parity):
/// başlık `Name | Ext | Size`, satırlar kutusuz metin, seçili satır yeşil
/// dolgu + koyu metin, taşmada sağda yeşil scrollbar.
fn draw_game_list(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let text_c = theme.color("text");
    let margin = ((w as f32 * 0.026) as u32).max(20);
    let top = (8 + HEADER_BLOCK_H + 8) as i32;
    if screen.games.is_empty() {
        let bw = ((w as i32) * 60 / 100).max(40) as u32;
        let bh: u32 = 48;
        let x = ((w as i32 - bw as i32) / 2).max(0) as i32;
        let y = (h as i32 / 2).max(0) as i32;
        canvas.set_draw_color(to_color(theme.color("button_idle")));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bw, bh));
        // Çekme sürüyorsa "yükleniyor", bittiyse "oyun yok" (dürüst boş durum).
        let msg = if screen.games_loading() {
            crate::i18n::tcached("loading_load_systems")
        } else {
            crate::i18n::tcached("game_no_games")
        };
        let _ = crate::text::draw_text_centered(canvas, tc, &msg, text_c, sdl2::rect::Rect::new(x, y, bw, bh), 12, font_scale);

        return;
    }
    // Kolonlar: Ext 80px, Size 130px sağda; Name kalanı.
    let ext_w: u32 = 80;
    let size_w: u32 = 130;
    let list_w = w.saturating_sub(margin * 2);
    let x0 = margin as i32;
    let panel = sdl2::rect::Rect::new(
        x0 - 10,
        top - 10,
        list_w + 20,
        (h.saturating_sub(margin + 40) as i32 - (top - 10)).max(60) as u32,
    );
    fill_rounded_rect(canvas, to_color(theme.color("button_idle")), panel, 12);
    draw_rounded_rect(canvas, to_color(theme.color("border")), panel, 12);
    let name_w = list_w.saturating_sub(ext_w + size_w + 32);
    let ext_x = x0 + name_w as i32 + 16;
    let size_x = x0 + list_w as i32 - size_w as i32;
    // Başlık satırı + ayraç.
    let head_h: u32 = 30;
    let _ = crate::text::draw_text(canvas, tc, "Name", text_c, x0 + 8, top + 6, 13, font_scale);
    let _ = crate::text::draw_text(canvas, tc, "Ext", text_c, ext_x, top + 6, 13, font_scale);
    let _ = crate::text::draw_text(canvas, tc, "Size", text_c, size_x, top + 6, 13, font_scale);
    canvas.set_draw_color(to_color(theme.color("border")));
    let _ = canvas.draw_line(
        (x0, top + head_h as i32),
        (x0 + list_w as i32, top + head_h as i32),
    );
    let row_h: u32 = 32;
    let gap: u32 = 4;
    let footer_top = h.saturating_sub(40) as i32;
    let avail_h = (footer_top - (top + head_h as i32 + 8)).max(0) as u32;
    let visible = ((avail_h / (row_h + gap)) as usize).max(1);
    // WebUI `filteredGames` parity: çizim + seçim aynı görünen listededir
    // (reducer ile aynı kaynak — highlight/Confirm tutarlı).
    let shown = screen.filtered_games();
    let sel = screen.selected_game.min(shown.len().saturating_sub(1));
    let total = shown.len();
    let start = sel.saturating_sub(visible / 2).min(total.saturating_sub(visible));
    let end = (start + visible).min(total);
    let mut y = top + head_h as i32 + 8;
    for (idx, g) in shown[start..end].iter().enumerate() {
        let abs_idx = start + idx;
        let is_sel = abs_idx == sel;
        let row_rect = sdl2::rect::Rect::new(x0, y, list_w, row_h);
        if is_sel {
            // Upstream `fond_lignes` (0,255,0) yeşil dolgu + koyu metin.
            canvas.set_draw_color(to_color(theme.color("fond_lignes")));
            let _ = canvas.fill_rect(row_rect);
        }
        if let Some(p) = screen.progress.get(&g.url) {
            // WebUI parity (`queuePct`): yalnız aktif durumlarda bar —
            // `Queued` bekleyenler boş bar illüzyonu vermez.
            let active = matches!(
                p.get("status").and_then(|v| v.as_str()).unwrap_or(""),
                "Downloading" | "Extracting" | "Connecting" | "Verifying" | "Seeding"
            );
            if active {
                if let Some(pct) = p.get("progress").and_then(|v| v.as_f64()) {
                    let fill_w = (list_w as f64 * (pct / 100.0).clamp(0.0, 1.0)) as u32;
                    if fill_w > 0 {
                        canvas.set_draw_color(to_color(theme.color("neon")));
                        let _ = canvas.fill_rect(sdl2::rect::Rect::new(x0, y, fill_w, 4));
                    }
                }
            }
        }
        let row_text = if is_sel { (10, 25, 10, 255) } else { text_c };
        // WebUI parity (`catalogStatus`): `[>]` indirildi / `[~] %` aktif /
        // `[X]` başarısız — öncelik indirilen > aktif > başarısız.
        let marker = crate::state::game_marker(&g.name, screen.progress.get(&g.url), &screen.downloaded);
        // Upstream: Name kolonunda uzantı YOK (Ext ayrı kolonda).
        let bare = g.name.strip_suffix(g.ext.as_str()).unwrap_or(g.name.as_str());
        let bare = match &marker {
            Some(m) => format!("{m} {bare}"),
            None => bare.to_string(),
        };
        let max_name = ((name_w.saturating_sub(16)) / 7).max(10) as usize;
        let name_disp = if bare.chars().count() > max_name {
            bare.chars().take(max_name.saturating_sub(3)).collect::<String>() + "..."
        } else {
            bare.to_string()
        };
        let _ = crate::text::draw_text(canvas, tc, &name_disp, row_text, x0 + 8, y + 7, 12, font_scale);
        let _ = crate::text::draw_text(canvas, tc, &g.ext, row_text, ext_x, y + 7, 12, font_scale);
        let _ = crate::text::draw_text(canvas, tc, &g.size, row_text, size_x, y + 7, 12, font_scale);
        y += (row_h + gap) as i32;
    }
    // Scrollbar (taşmada): sağda yeşil başparmak.
    if total > visible {
        let track_x = x0 + list_w as i32 + 6;
        let track_y = top + head_h as i32 + 8;
        let track_h = (visible as u32 * (row_h + gap)) as i32;
        canvas.set_draw_color(to_color(theme.color("border")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(track_x, track_y, 6, track_h as u32));
        let frac = total as f32;
        let thumb_h = ((visible as f32 / frac) * track_h as f32) as u32;
        let thumb_y = track_y + ((start as f32 / frac) * track_h as f32) as i32;
        canvas.set_draw_color(to_color(theme.color("fond_lignes")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(track_x, thumb_y, 6, thumb_h.max(12)));
    }
}
/// Footer öğesi — gamepad/keycap parity için yapısal form.
/// `cap` tuş başlığı (H/F/Enter/Esc), `label` yerelleşmiş eylem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FooterItem {
    pub cap: String,
    pub label: String,
}

/// SDL'siz footer öğeleri (Python `render_combined_footer_controls` parity).
/// Çizim `draw_footer`'da keycap rozeti + etiket olarak dizilir.
pub fn footer_items(menu: &MenuState) -> Vec<FooterItem> {
    use crate::i18n::tcached as t;
    let item = |cap: &str, label: String| FooterItem {
        cap: cap.to_string(),
        label,
    };
    match menu {
        // Dürüst sözleşme: yalnız GERÇEKTEN çalışan tuşlar yazılır.
        // Geçmiş ekranı yok → H maddesi yok; long-press yok → ikinci Enter yok.
        MenuState::PlatformGrid => vec![
            item("Enter", t("controls_confirm_select")),
            item("AltGR", t("controls_action_start")),
        ],
        MenuState::GameList => vec![
            item("Enter", t("controls_confirm_select")),
            item("X", t("controls_action_queue")),
            FooterItem {
                cap: "[Page+][Page-]".to_string(),
                label: t("controls_pages"),
            },
            item("F", t("controls_filter_search")),
            item("M", t("controls_action_start")),
        ],
        MenuState::Loading => vec![
            item("R", "Retry".to_string()),
            item("Enter", "Offline".to_string()),
        ],
        MenuState::Error(_) => vec![
            item("R", "Retry".to_string()),
            item("Enter", "Offline".to_string()),
            item("Esc", t("controls_cancel_back")),
        ],
        MenuState::ConfirmExit => vec![
            item("Enter", t("controls_confirm_select")),
            item("Esc", t("controls_cancel_back")),
        ],
    }
}

/// Footer yerleşimi — SDL'siz, test edilebilir.
/// Her öğe `cap rozeti + etiket` genişliğiyle yan yana dizilir; genişliği
/// aşan kuyruk öğeler atılır (sonuna "..." değil, başa sığanlar korunur —
/// upstream'de ilk aksiyonlar en kritik). Dönüş: ekrana sığan dilim.
pub fn footer_layout(
    w: u32,
    font_scale: f32,
    items: &[FooterItem],
) -> Vec<FooterItem> {
    if items.is_empty() || w < 60 {
        return Vec::new();
    }
    let avail = w.saturating_sub(40) as f32;
    let mut used = 0.0f32;
    let mut out = Vec::new();
    for it in items {
        // Kaba metrik: cap ~8px/karakter + 16px rozet padding, label ~7px/karakter + 10px gap.
        // 14px DejaVu ortalaması + font_scale (draw_footer ile aynı formül).
        let cap_w = (it.cap.chars().count() as f32 * 8.0 + 16.0) * font_scale;
        let label_w = (it.label.chars().count() as f32 * 7.0 + 10.0) * font_scale;
        let need = cap_w + label_w + 18.0 * font_scale; // öğeler arası boşluk
        if used + need > avail && !out.is_empty() {
            break;
        }
        used += need;
        out.push(it.clone());
    }
    if out.is_empty() {
        // En dar ekranda bile ilk öğenin cap'i görünsün.
        out.push(items[0].clone());
    }
    out
}

/// Footer satırını alta çizer — gamepad/keycap rozetleri + etiket.
/// Her `cap` yuvarlak rozette (button_idle dolgu + border çerçeve), etiket
/// beyaz metin. Dar ekranda `footer_layout` kuyruğu atar (ilk aksiyon korunur).
fn draw_footer(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let items = footer_layout(w, font_scale, &footer_items(&screen.menu));
    if items.is_empty() {
        return;
    }
    let bh: u32 = 30;
    let y = h.saturating_sub(bh) as i32;
    let mut x = 20i32;
    for it in &items {
        // Keycap rozeti: metrik ile aynı formül (çizim/yerleşim tutarlı).
        let cap_w = ((it.cap.chars().count() as f32 * 8.0 + 16.0) * font_scale) as u32;
        let cap_h: u32 = 22;
        let cap_rect = sdl2::rect::Rect::new(x, y + ((bh as i32 - cap_h as i32) / 2).max(0), cap_w.max(24), cap_h);
        fill_rounded_rect(canvas, to_color(theme.color("button_idle")), cap_rect, 6);
        draw_rounded_rect(canvas, to_color(theme.color("border")), cap_rect, 6);
        let _ = crate::text::draw_text_centered(
            canvas,
            tc,
            &it.cap,
            theme.color("text"),
            cap_rect,
            11,
            font_scale,
        );
        x += cap_rect.width() as i32 + 6;
        let label_w = ((it.label.chars().count() as f32 * 7.0 + 8.0) * font_scale) as u32;
        let _ = crate::text::draw_text(
            canvas,
            tc,
            &it.label,
            theme.color("text"),
            x,
            y + 4,
            13,
            font_scale,
        );
        x += label_w as i32 + (18.0 * font_scale) as i32;
        if x > w as i32 - 20 {
            break;
        }
    }
}

/// TASK-012i — pause menu overlay (Faz 1: rect + highlight, metin TTF ile sonra).
fn draw_menu_overlay(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let Some(ov) = &screen.overlay else {
        return;
    };
    canvas.set_draw_color(to_color(theme.color("shadow")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, h));
    let bw = ((w as i32) * 50 / 100).max(200) as u32;
    let bh_each: u32 = 40;
    let gap: u32 = 8;
    let total_h = ov.items.len() as u32 * bh_each + (ov.items.len().saturating_sub(1) as u32 * gap) + 20;
    let bx = ((w as i32 - bw as i32) / 2).max(0) as i32;
    let by = ((h as i32 - total_h as i32) / 2).max(0) as i32;
    let panel = sdl2::rect::Rect::new(bx, by, bw, total_h);
    fill_rounded_rect(canvas, to_color(theme.color("button_idle")), panel, 12);
    draw_rounded_rect(canvas, to_color(theme.color("border")), panel, 12);
    for (i, label) in ov.items.iter().enumerate() {
        let y = by + 10 + i as i32 * (bh_each as i32 + gap as i32);
        let is_sel = i == ov.selected;
        let bg = if is_sel {
            theme.color("button_selected")
        } else {
            theme.color("button_idle")
        };
        let border = if is_sel {
            theme.color("border_selected")
        } else {
            theme.color("border")
        };
        let r = sdl2::rect::Rect::new(bx + 10, y, bw - 20, bh_each);
        fill_rounded_rect(canvas, to_color(bg), r, 10);
        draw_rounded_rect(canvas, to_color(border), r, 10);
        let _ = crate::text::draw_text_centered(canvas, tc, label, theme.color("text"), r, 12, font_scale);

    }
}

/// TASK-012j — sanal klavye overlay (gamepad ızgara, seçili tuş border_selected).
fn draw_virtual_keyboard(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let Some(kb) = &screen.keyboard else { return; };
    canvas.set_draw_color(to_color(theme.color("shadow")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, h));
    let iw = ((w as i32) * 60 / 100).max(200) as u32;
    let ih: u32 = 36;
    let ix = ((w as i32 - iw as i32) / 2).max(0) as i32;
    let iy = (h as i32 / 6).max(0) as i32;
    canvas.set_draw_color(to_color(theme.color("button_selected")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(ix, iy, iw, ih));
    canvas.set_draw_color(to_color(theme.color("border_selected")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(ix, iy, iw, ih));
    let _ = crate::text::draw_text(canvas, tc, &kb.input, theme.color("neon"), ix + 8, iy + 8, 13, font_scale);

    let key_w: u32 = 48;
    let key_h: u32 = 40;
    let gap: u32 = 6;
    let rows = kb.layout.len() as u32;
    let total_h = rows * key_h + (rows.saturating_sub(1) * gap) + 20;
    let ky = iy + ih as i32 + 20;
    let _ = total_h;
    for (r, row) in kb.layout.iter().enumerate() {
        let row_w = row.len() as u32 * key_w + (row.len().saturating_sub(1) as u32 * gap);
        let rx = ((w as i32 - row_w as i32) / 2).max(0) as i32;
        let ry = ky + r as i32 * (key_h as i32 + gap as i32);
        if ry + key_h as i32 > h as i32 || ry < 0 { continue; }
        for (c, k) in row.iter().enumerate() {
            let x = rx + c as i32 * (key_w as i32 + gap as i32);
            let is_sel = kb.cursor == (r, c);
            let bg = if is_sel { theme.color("button_selected") } else { theme.color("button_idle") };
            let border = if is_sel { theme.color("border_selected") } else { theme.color("border") };
            canvas.set_draw_color(to_color(bg));
            let _ = canvas.fill_rect(sdl2::rect::Rect::new(x, ry, key_w, key_h));
            canvas.set_draw_color(to_color(border));
            let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, ry, key_w, key_h));
            let _ = crate::text::draw_text_centered(canvas, tc, k, theme.color("neon"), sdl2::rect::Rect::new(x, ry, key_w, key_h), 11, font_scale);

        }
    }
}

/// TASK-012m Faz 5 — self-update banner (fontdue metinsiz sürümle uyumlu kutu çizimi).
/// Aşamaya göre renk: `available`=warning_text (turuncu), `downloading`=neon
/// (mavi, iç dolgu=percent), `ready`=success (yeşil), `failed`=error_text (kırmızı).
fn draw_update_banner(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    (w, _h): (u32, u32),
) {
    let (avail, stage, pct) = {
        let s = tvui_lock(state);
        (
            s.update_available.clone(),
            s.update_stage.clone(),
            s.update_pct,
        )
    };
    let Some(ver) = avail else {
        return;
    };
    let stage = stage.unwrap_or_else(|| "available".to_string());
    let color_key = match stage.as_str() {
        "ready" => "fond_lignes",
        "downloading" => "neon",
        "failed" => "error_text",
        _ => "warning_text",
    };
    let bw = ((w as i32) * 60 / 100).max(40) as u32;
    let bx = ((w as i32 - bw as i32) / 2).max(0) as i32;
    let by = 8i32;
    let bh: u32 = 28;
    let color = to_color(theme.color(color_key));
    canvas.set_draw_color(color);
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(bx, by, bw, bh));
    if stage == "downloading" {
        let fill_w = ((bw as u64 * pct as u64 / 100) as u32).max(1).min(bw);
        canvas.set_draw_color(color);
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx, by, fill_w, bh));
    }
    if stage == "ready" {
        canvas.set_draw_color(color);
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx, by, bw, bh));
    }
    let _ = ver;
}

/// TASK-012m Faz 5 — apply sonrası "Yeniden başlatılıyor…" tam ekran overlay'i
/// (yalnız ayrı bir renk katmanı).
fn draw_restart_screen(canvas: &mut Canvas<Window>, theme: &Theme, (w, h): (u32, u32)) {
    let c = to_color(theme.color("neon"));
    canvas.set_draw_color(c);
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, h));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(
        (w as i32 / 4).max(0),
        (h as i32 / 4).max(0),
        (w / 2).max(1),
        (h / 2).max(1),
    ));
}

/// TASK-012j — folder browser overlay (path + liste + scrollbar + seçili highlight).
fn draw_folder_browser(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let Some(fb) = &screen.browser else { return; };
    canvas.set_draw_color(to_color(theme.color("shadow")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, h));
    let pw = ((w as i32) * 80 / 100).max(200) as u32;
    let ph = ((h as i32) * 85 / 100).max(200) as u32;
    let px = ((w as i32 - pw as i32) / 2).max(0) as i32;
    let py = ((h as i32 - ph as i32) / 2).max(0) as i32;
    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(px, py, pw, ph));
    canvas.set_draw_color(to_color(theme.color("border")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(px, py, pw, ph));
    let bar_h: u32 = 28;
    canvas.set_draw_color(to_color(theme.color("button_selected")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(px + 10, py + 40, pw - 20, bar_h));
    canvas.set_draw_color(to_color(theme.color("border_selected")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(px + 10, py + 40, pw - 20, bar_h));
    // stale ttf guard kaldırıldı — fontdue dahili fallback kullanıyor
        let path_txt = fb.current_path.display().to_string();
        let truncated = if path_txt.chars().count() > 60 { format!("...{}", path_txt.chars().skip(path_txt.chars().count() - 57).collect::<String>()) } else { path_txt };
        let _ = crate::text::draw_text(canvas, tc, &truncated, theme.color("neon"), px + 14, py + 44, 11, font_scale);

    let list_y = py + 40 + bar_h as i32 + 10;
    let row_h: u32 = 28;
    let gap: u32 = 4;
    let visible = ((ph - 80) / (row_h + gap)) as usize;
    let start_idx = fb.selection.saturating_sub(visible / 2).min(fb.items.len().saturating_sub(visible));
    let end_idx = (start_idx + visible).min(fb.items.len());
    let mut y = list_y;
    for (idx, entry) in fb.items[start_idx..end_idx].iter().enumerate() {
        let abs_idx = start_idx + idx;
        let is_sel = abs_idx == fb.selection;
        let bg = if is_sel { theme.color("button_selected") } else { theme.color("button_idle") };
        let border = if is_sel { theme.color("border_selected") } else { theme.color("border") };
        canvas.set_draw_color(to_color(bg));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(px + 10, y, pw - 20, row_h));
        canvas.set_draw_color(to_color(border));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(px + 10, y, pw - 20, row_h));
        // stale ttf guard kaldırıldı — fontdue dahili fallback kullanıyor
            let name = entry.clone();
            let _ = crate::text::draw_text(canvas, tc, &name, theme.color("neon"), px + 14, y + 6, 11, font_scale);

        y += (row_h + gap) as i32;
    }
    // Scrollbar
    if fb.items.len() > visible {
        let bar_track_h = (visible as u32 * (row_h + gap)) as i32;
        let thumb_h = ((visible as f32 / fb.items.len() as f32) * bar_track_h as f32) as u32;
        let thumb_y = list_y + ((start_idx as f32 / fb.items.len() as f32) * bar_track_h as f32) as i32;
        canvas.set_draw_color(to_color(theme.color("border")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(px + pw as i32 - 10, list_y, 6, bar_track_h as u32));
        canvas.set_draw_color(to_color(theme.color("neon")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(px + pw as i32 - 10, thumb_y, 6, thumb_h.max(10)));
    }
}

pub fn run_native_shell(
    theme: &Theme,
    state: &SharedTvuiState,
    shutdown: &AtomicBool,
) -> Result<(), String> {
    let sdl = sdl2::init().map_err(|e| format!("SDL2 init: {e}"))?;
    // TTF: fontdue pure-Rust (sdl2_ttf bagimliligi kaldirildi)
    let video = sdl.video().map_err(|e| format!("SDL2 video: {e}"))?;
    // Bulgu 13: `RGSX_TVUI_WINDOWED=1` → resizable pencere (masaüstü test/debug);
    // varsayılan 10-foot fullscreen kalır (Python `get_display_fullscreen()` parity'si
    // tam ayar menüsüyle TASK-012e/k'ta).
    let windowed = std::env::var("RGSX_TVUI_WINDOWED")
        .map(|v| v == "1")
        .unwrap_or(false);
    let mut wb = video.window("RGSX", 1280, 720);
    wb.position_centered();
    if windowed {
        wb.resizable();
    } else {
        wb.fullscreen();
    }
    let window = wb.build().map_err(|e| format!("SDL2 pencere: {e}"))?;
    // Bulgu 12: vsync tercih edilir; gerçek durum renderer info'sundan okunur —
    // vsync yoksa eski 16 ms sleep devrede kalır.
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .present_vsync()
        .build()
        .map_err(|e| format!("SDL2 canvas: {e}"))?;
    let vsync =
        canvas.info().flags & sdl2::sys::SDL_RendererFlags::SDL_RENDERER_PRESENTVSYNC as u32 != 0;
    let texture_creator = canvas.texture_creator();
    let mut bg_cache: Option<(u32, u32, Texture)> = None;
    let mut event_pump = sdl.event_pump().map_err(|e| format!("SDL2 event: {e}"))?;

    let preset = std::env::var("RGSX_TVUI_BG").unwrap_or_else(|_| "default".into());
    // Box-art kökü: tema `assets/icons/` göreli yazar; gerçek logolar data_dir/images
    // altındadır (katalogla aynı kaynak). Çözülmüş yolla temanın kopyasını kullan.
    let mut resolved_theme = theme.clone();
    resolved_theme.icons.path = resolve_icons_path(&theme.icons.path);
    let theme: &Theme = &resolved_theme;
    // TASK-012h Faz 3/5 + gap-05: state machine + box-art texture cache (cap 64).
    let mut screen = TvuiScreen::default();
    let mut art_cache =
        crate::boxart::SdlBoxArtCache::new(crate::boxart::MAX_TEXTURES, theme.icons.path.clone());

    'running: loop {
        if shutdown.load(Ordering::Relaxed) {
            break 'running; // Faz C bulgu 9: gamepad back.
        }
        // SSE net state'ini screen'e senkronla (loading/ready/error/offline/platforms)
        {
            let net = tvui_lock(state).clone();
            screen.net = net.clone();
            if screen.platforms.is_empty() && !net.platforms.is_empty() {
                screen.platforms = net.platforms.clone();
            } else if !net.platforms.is_empty() && screen.platforms.len() != net.platforms.len() {
                screen.platforms = net.platforms.clone();
                if screen.selected_platform >= screen.platforms.len() {
                    screen.selected_platform = 0;
                }
            }
            screen.sync_from_net();
        }
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    // Faz 3: Esc → Back (ConfirmExit) reducer üzerinden
                    let now = std::time::Instant::now();
                    let _ = crate::state::reduce(&mut screen, UiKey::Back, now);
                    if matches!(screen.menu, MenuState::ConfirmExit) {
                        // İkinci Esc veya ConfirmExit'te çıkış
                        break 'running;
                    }
                }
                Event::KeyDown {
                    keycode: Some(kc), ..
                } => {
                    let now = std::time::Instant::now();
                    // Net UiAction'lar (Retry/Confirm/CancelUpdate) önce dene
                    let net_key = match kc {
                        Keycode::R => Some(UiKey::Retry),
                        Keycode::Return | Keycode::KpEnter => Some(UiKey::Confirm),
                        Keycode::C => Some(UiKey::CancelUpdate),
                        _ => None,
                    };
                    if let Some(k) = net_key {
                        let action = {
                            let s = tvui_lock(state);
                            ui_decision(&s, k)
                        };
                        if let Some(action) = action {
                            apply_ui_action(state, action);
                            continue;
                        }
                    }
                    // Nav/page/Back/Menu/Queue → state reducer (Faz 3+4+012i)
                    // WebUI parity: X tek-buton indirmenin ikinci tetikleyicisidir.
                    // AltGr (Windows: RAlt; bazı layoutlarda Mode) gamepad Start
                    // ile aynıdır → pause/ayar menüsü (`native_input` "start").
                    let nav_key = match kc {
                        Keycode::Up => Some(UiKey::NavUp),
                        Keycode::Down => Some(UiKey::NavDown),
                        Keycode::Left => Some(UiKey::NavLeft),
                        Keycode::Right => Some(UiKey::NavRight),
                        Keycode::PageUp => Some(UiKey::PageUp),
                        Keycode::PageDown => Some(UiKey::PageDown),
                        Keycode::Backspace => Some(UiKey::Back),
                        Keycode::M | Keycode::RAlt | Keycode::Mode => Some(UiKey::Menu),
                        Keycode::F => Some(UiKey::Search),
                        Keycode::Return | Keycode::KpEnter => Some(UiKey::Confirm),
                        Keycode::X => Some(UiKey::Queue),
                        _ => None,
                    };
                    if let Some(k) = nav_key {
                        let prev_menu = screen.menu.clone();
                        if let Some(action) = crate::state::reduce(&mut screen, k, now) {
                            apply_ui_action(state, action);
                        }
                        // Faz 4: PlatformGrid→GameList geçişinde oyunları HER ZAMAN
                        // taze çek (koşulsuz — bayat liste gösterilmez; el sıkışma
                        // `games_platform` ile eşleşene kadar liste boş+loading).
                        if matches!(prev_menu, MenuState::PlatformGrid)
                            && matches!(screen.menu, MenuState::GameList)
                        {
                            let plat = screen.games_platform.clone();
                            if !plat.is_empty() {
                                let st = Arc::clone(state);
                                std::thread::spawn(move || {
                                    // Önce bayrakları düşür (çekme-sürüyor durumu),
                                    // sonra WebUI `selectPlatform` parity: oyunlar +
                                    // indirilen durumları birlikte çekilir.
                                    {
                                        let mut s = tvui_lock(&st);
                                        s.games_ready = false;
                                        s.statuses_ready = false;
                                        s.games.clear();
                                    }
                                    let games = crate::net::fetch_games(tvui_lock(&st).port, &plat);
                                    let statuses = crate::net::fetch_game_statuses(tvui_lock(&st).port);
                                    let mut s = tvui_lock(&st);
                                    s.games = games;
                                    s.games_platform = plat.clone();
                                    s.games_ready = true;
                                    if let Some(dl) = statuses {
                                        s.downloaded = dl;
                                        s.statuses_platform = plat;
                                        s.statuses_ready = true;
                                    }
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // Bulgu 7: relaunch süreci devralmadıysa overlay'i kapat (ölü ekran koruması).
        {
            let mut s = tvui_lock(state);
            expire_stale_restart_at(&mut s, std::time::Instant::now());
        }
        let dims = draw_background(&mut canvas, &texture_creator, &mut bg_cache, theme, &screen, &preset);
        draw_update_banner(&mut canvas, theme, state, dims);
        // Yeniden başlatma ekranı (apply sonrası) — grid/loading yerine tam ekran.
        let restarting = tvui_lock(state).update_restarting;
        if restarting {
            draw_restart_screen(&mut canvas, theme, dims);
        } else {
            // screen.menu üzerinden çizim (Faz 3/4/5)
                let font_scale = screen.a11y.font_scale().max(0.5).min(3.0);
    match screen.menu {
                MenuState::PlatformGrid => {
                    art_cache.sync_icons_path(&theme.icons.path);
                    draw_grid(&mut canvas, theme, state, &screen, dims, &texture_creator, font_scale, &mut art_cache)
                },
                MenuState::GameList => draw_game_list(&mut canvas, theme, &screen, dims, &texture_creator, font_scale),
                MenuState::Loading | MenuState::Error(_) => draw_loading(&mut canvas, theme, state, dims, &texture_creator, font_scale),
                MenuState::ConfirmExit => {
                    art_cache.sync_icons_path(&theme.icons.path);
                    draw_grid(&mut canvas, theme, state, &screen, dims, &texture_creator, font_scale, &mut art_cache);
                    canvas.set_draw_color(to_color(theme.color("warning_text")));
                    let _ = canvas.draw_rect(sdl2::rect::Rect::new(0, 0, dims.0, dims.1));
                }
            }
            draw_header(&mut canvas, theme, &screen, dims, &texture_creator, font_scale);
            let footer_scale = screen.a11y.footer_font_scale().max(0.5).min(3.0);
            draw_footer(&mut canvas, theme, &screen, dims, &texture_creator, footer_scale);
            // TASK-012i: overlay varsa üstte çiz (pause/display/filter)
            if screen.overlay.is_some() {
                draw_menu_overlay(&mut canvas, theme, &screen, dims, &texture_creator, font_scale);
            }
            // TASK-012j: sanal klavye / folder browser en üstte (overlay üstüne)
            if screen.keyboard.is_some() {
                draw_virtual_keyboard(&mut canvas, theme, &screen, dims, &texture_creator, font_scale);
            }
            if screen.browser.is_some() {
                draw_folder_browser(&mut canvas, theme, &screen, dims, &texture_creator, font_scale);
            }
            // Faz 5: transition bittiyse temizle
            if let Some(tr) = &screen.transition {
                if tr.is_finished(Instant::now()) {
                    screen.transition = None;
                }
            }
        }
        canvas.present();
        // Bulgu 12: vsync aktifse present zaten yenileme hızını kısıtlar;
        // sleep'i yalnız vsync'siz durumda tut (30 fps'e düşme hatası olmasın).
        if !vsync {
            std::thread::sleep(Duration::from_millis(16));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_icons_path_prefers_images_folder_env() {
        let prev_img = std::env::var("RGSX_IMAGES_FOLDER").ok();
        let prev_data = std::env::var("RGSX_DATA_DIR").ok();
        let tmp = std::env::temp_dir().join("rgsx-tvui-test-images");
        std::fs::create_dir_all(&tmp).unwrap();
        std::env::set_var("RGSX_IMAGES_FOLDER", &tmp);
        std::env::remove_var("RGSX_DATA_DIR");
        assert_eq!(resolve_icons_path("assets/icons/"), tmp.to_string_lossy());
        match prev_img {
            Some(v) => std::env::set_var("RGSX_IMAGES_FOLDER", v),
            None => std::env::remove_var("RGSX_IMAGES_FOLDER"),
        }
        match prev_data {
            Some(v) => std::env::set_var("RGSX_DATA_DIR", v),
            None => std::env::remove_var("RGSX_DATA_DIR"),
        }
        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn resolve_icons_path_falls_back_to_theme() {
        let prev_img = std::env::var("RGSX_IMAGES_FOLDER").ok();
        let prev_data = std::env::var("RGSX_DATA_DIR").ok();
        std::env::remove_var("RGSX_IMAGES_FOLDER");
        std::env::set_var(
            "RGSX_DATA_DIR",
            std::env::temp_dir().join("rgsx-tvui-test-nodir"),
        );
        // exe-anchor da tutmazsa tema yolu aynen döner (çağıran kutuyu korur).
        let out = resolve_icons_path("assets/icons/");
        assert!(
            out == "assets/icons/"
                || out.ends_with("saves/ports/rgsx/images")
                || out.ends_with("saves\\ports\\rgsx\\images"),
            "beklenmeyen: {out}"
        );
        match prev_img {
            Some(v) => std::env::set_var("RGSX_IMAGES_FOLDER", v),
            None => std::env::remove_var("RGSX_IMAGES_FOLDER"),
        }
        match prev_data {
            Some(v) => std::env::set_var("RGSX_DATA_DIR", v),
            None => std::env::remove_var("RGSX_DATA_DIR"),
        }
    }

    #[test]
    fn format_disk_gb_matches_python_tiers() {
        assert_eq!(format_disk_gb(349 * 1024 * 1024 * 1024), "349 GB");
        assert_eq!(format_disk_gb((12.5 * 1024.0 * 1024.0 * 1024.0) as u64), "12.5 GB");
        assert_eq!(format_disk_gb((1.23 * 1024.0 * 1024.0 * 1024.0) as u64), "1.23 GB");
    }

    #[test]
    fn app_version_matches_version_json() {
        assert_eq!(app_version(), "2.6.5.8");
    }

    #[test]
    fn format_page_with_pattern() {
        assert_eq!(format_page_with("Page {0}/{1}", 0, 13), "Page 1/13");
        assert_eq!(format_page_with("Sayfa {0}/{1}", 2, 5), "Sayfa 3/5");
        assert_eq!(format_page_with("no-placeholders", 0, 1), "Page 1/1");
    }

    #[test]
    fn header_data_shapes_upstream_badges() {
        let disk = "[HDD] 241 GB/446 GB (54% free)";
        let (l, m, r) = header_data("BIOS", 13, Some("Page 1/13"), None, 1600, 900, disk, "2.6.5.8", "10.0.0.36");
        assert_eq!(l, vec!["Page 1/13".to_string(), disk.to_string(), "Res : 1600x900".to_string()]);
        assert_eq!(m, "BIOS  (13)");
        assert_eq!(r, vec!["v2.6.5.8".to_string(), "10.0.0.36".to_string()]);
        // Tek sayfa: Page satırı yok; boş platform → RGSX fallback.
        let (l2, m2, _) = header_data("", 0, None, None, 800, 600, "", "2.6.5.8", "127.0.0.1");
        assert_eq!(l2, vec!["Res : 800x600".to_string()]);
        assert_eq!(m2, "RGSX  (0)");
        // Oyun listesi: "(N games)" eki.
        let (_, m3, _) = header_data("Archimedes (Archive)", 72, None, Some("games"), 1600, 900, disk, "2.6.5.8", "10.0.0.36");
        assert_eq!(m3, "Archimedes (Archive) (72 games)");
    }

    #[test]
    fn disk_line_empty_path_is_empty() {
        assert_eq!(disk_line(""), "");
    }

    #[test]
    fn rounded_fill_spans_shape() {
        // Boş / sıfır
        assert!(rounded_fill_spans(0, 10, 5).is_empty());
        // r=0 → tam dikdörtgen
        let full = rounded_fill_spans(10, 4, 0);
        assert_eq!(full.len(), 4);
        assert!(full.iter().all(|&(_, x0, x1)| x0 == 0 && x1 == 10));
        // Simetri + köşe girintisi
        let spans = rounded_fill_spans(100, 40, 12);
        assert_eq!(spans.len(), 40);
        assert_eq!(spans[20].1, 0); // orta satır tam genişlik
        assert!(spans[0].1 > 0); // üst köşe girintili
        assert_eq!(spans[0], (0, spans[39].1, spans[39].2)); // dikey simetri... ilk/son
        assert_eq!(spans[0].1, spans[39].1);
        // Clamp: r > min(w,h)/2
        let cl = rounded_fill_spans(20, 20, 99);
        assert_eq!(cl.len(), 20);
    }

    #[test]
    fn source_badge_key_mapping() {
        assert_eq!(source_badge_key("3DS (Archive)"), Some("Archive"));
        assert_eq!(source_badge_key("3DS (Vimms)"), Some("Vimms"));
        assert_eq!(source_badge_key("Game (edgeemu.net)"), Some("EdgeEmu"));
        assert_eq!(source_badge_key("Game (1Fichier)"), Some("1Fichier"));
        assert_eq!(source_badge_key("NoParen"), None);
        assert_eq!(source_badge_key(""), None);
        assert_eq!(source_badge_key("Weird (Unknown)"), None);
        let (label, _, _) = source_badge_style("Archive").unwrap();
        assert_eq!(label, "AR");
        assert!(source_badge_style("Bogus").is_none());
    }

    #[test]
    fn footer_line_has_keycaps_per_menu() {
        // Tuş başlıkları yerelden bağımsız; etiketler t() ile gelir.
        // Tek-satır gösterim `footer_items`'tan türetilir (çizimle aynı kaynak).
        let line = |menu: &MenuState| {
            footer_items(menu)
                .iter()
                .map(|it| format!("[{}] : {}", it.cap.trim_matches(|c| c == '[' || c == ']'), it.label))
                .collect::<Vec<_>>()
                .join("  ")
        };
        let grid = line(&MenuState::PlatformGrid);
        for cap in ["[Enter] :", "[AltGR] :"] {
            assert!(grid.contains(cap), "yok: {cap} ({grid})");
        }
        assert!(!grid.contains("[H] :"), "ölü H maddesi: {grid}");
        let game = line(&MenuState::GameList);
        for cap in ["[Enter] :", "[X] :", "[Page+][Page-] :", "[F] :", "[M] :"] {
            assert!(game.contains(cap), "yok: {cap} ({game})");
        }
        assert!(!game.contains("[H] :"), "ölü H maddesi: {game}");
        assert!(line(&MenuState::ConfirmExit).contains("[Esc] :"));
    }

    #[test]
    fn selection_pulse_stays_in_band_and_loops() {
        // Bant: 1.10..=1.20, 600ms periyotla başa döner, determinist.
        for ms in [0u64, 150, 300, 450, 599] {
            let s = selection_pulse_scale(ms);
            assert!((1.10..=1.20).contains(&s), "bant dışı {ms}: {s}");
        }
        assert!((selection_pulse_scale(0) - selection_pulse_scale(600)).abs() < 1e-6);
        assert!((selection_pulse_scale(150) - 1.20).abs() < 1e-6); // tepe
        assert!((selection_pulse_scale(450) - 1.10).abs() < 1e-6); // çukur
    }

    #[test]
    fn footer_items_structured_per_menu() {
        let grid = footer_items(&MenuState::PlatformGrid);
        assert_eq!(grid.len(), 2);
        assert_eq!(grid[0].cap, "Enter");
        assert_eq!(grid[1].cap, "AltGR");
        let gl = footer_items(&MenuState::GameList);
        assert!(gl.iter().any(|it| it.cap == "F"));
        assert!(gl.iter().any(|it| it.cap == "M"));
        assert!(!gl.iter().any(|it| it.cap == "H"));
        assert_eq!(footer_items(&MenuState::ConfirmExit).len(), 2);
        assert_eq!(footer_items(&MenuState::ConfirmExit)[0].cap, "Enter");
        // Esc cap'i çizimde rozet olur; veri burada doğrulanır.
        assert!(footer_items(&MenuState::ConfirmExit).iter().any(|it| it.cap == "Esc"));
    }

    #[test]
    fn footer_layout_truncates_tail_on_narrow() {
        let items = footer_items(&MenuState::PlatformGrid);
        let wide = footer_layout(1600, 1.0, &items);
        assert_eq!(wide.len(), items.len());
        let narrow = footer_layout(320, 1.0, &items);
        assert!(!narrow.is_empty());
        assert!(narrow.len() < items.len());
        assert_eq!(narrow[0].cap, items[0].cap); // ilk aksiyon korunur
    }

    #[test]
    fn loading_texts_show_stage_and_pct() {
        let (title, sub) = loading_texts(42, "download", None);
        assert_eq!(title, "download");
        assert_eq!(sub, "42%");
        let (t2, _) = loading_texts(0, "", None);
        assert_eq!(t2, "Yükleniyor");
        let (te, se) = loading_texts(7, "", Some("boom"));
        assert_eq!(te, "Hata");
        assert!(se.contains("boom") && se.contains("7%"));
        let (px, py, pw, ph, bx, by, bw, bh) = loading_layout(1280, 720);
        assert!(pw > bw && ph > bh && px >= 0 && py >= 0);
        assert_eq!((bx, by), ((1280i32 - bw as i32) / 2, 720 / 2));
        let _ = (px, py);
    }
}
