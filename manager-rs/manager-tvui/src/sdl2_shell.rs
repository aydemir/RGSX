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
use crate::state::{MenuState, TvuiScreen};
use crate::theme::Theme;

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
        let mut pixels: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let t = if h <= 1 {
                0.0
            } else {
                y as f32 / (h - 1) as f32
            };
            pixels.push(lerp(top.0, bottom.0, t));
            pixels.push(lerp(top.1, bottom.1, t));
            pixels.push(lerp(top.2, bottom.2, t));
            pixels.push(255);
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

/// Açılış loading bar'ı: SSE `catalog_update` ilerlemesini `state`'ten okur.
/// `ready` oluncaya kadar (ya da hata varsa) ekranın ortasında çubuk çizer.
/// Hata varsa belirgin kırmızı çerçeve çizer (metin yok — TTF erte).
fn draw_loading(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let (pct, error) = {
        let s = tvui_lock(state);
        (s.pct.clamp(0, 100) as f32 / 100.0, s.error.clone())
    };
    let bar_w = ((w as i32) * 60 / 100).max(40) as u32;
    let bar_h: u32 = 24;
    let x = ((w as i32 - bar_w as i32) / 2).max(0) as i32;
    let y = (h as i32 / 2).max(0) as i32;

    if let Some(err) = &error {
        canvas.set_draw_color(to_color(theme.color("error_text")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, (y - 40).max(0) as i32, w, 6));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
        // stale ttf guard kaldırıldı — fontdue dahili fallback kullanıyor
            let msg = format!("{} ({}%)", err.chars().take(60).collect::<String>(), (pct * 100.0) as i32);
            let _ = crate::text::draw_text_centered(canvas, tc, &msg, theme.color("error_text"), sdl2::rect::Rect::new(x, y + bar_h as i32 + 8, bar_w, 28), 13, font_scale);

        return;
    }

    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
    let fill_w = (bar_w as f32 * pct) as i32;
    if fill_w > 0 {
        canvas.set_draw_color(to_color(theme.color("neon")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(x, y, fill_w as u32, bar_h));
    }
    // stale ttf guard kaldırıldı — fontdue dahili fallback kullanıyor
        let msg = format!("{}%", (pct * 100.0) as i32);
        let _ = crate::text::draw_text_centered(canvas, tc, &msg, theme.color("neon"), sdl2::rect::Rect::new(x, y + bar_h as i32 + 8, bar_w, 24), 14, font_scale);

}

/// `ready` sonrası platform grid'i: `/api/platforms`'tan gelen `state.platforms`
/// listesini tile olarak dizer. Faz 3: seçili tile `border_selected` + scale ile vurgulanır.
fn draw_grid<'a>(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    screen: &TvuiScreen,
    (w, _h): (u32, u32),
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
    let cols: u32 = 6;
    let gap: u32 = 16;
    let margin: u32 = 40;
    let avail_w = w.saturating_sub(margin * 2);
    let tile_w = (avail_w.saturating_sub(gap * (cols - 1))) / cols;
    let tile_h = tile_w * 3 / 4;
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
    for (i, p) in platforms.iter().enumerate() {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let base_x = margin + col * (tile_w + gap);
        let base_y = margin + row * (tile_h + gap);
        let is_sel = sel == Some(i);
        // Box-art cache: ikon yolunu çöz (folder bazlı, platform_image fallback)
        let icon_path = crate::render::BoxArtCache::icon_path_for(&p.folder, &theme.icons.path);
        // Seçili tile: transition scale + border_selected
        if is_sel {
            let scale = trans_scale;
            let sw = (tile_w as f32 * scale) as u32;
            let sh = (tile_h as f32 * scale) as u32;
            let dx = ((tile_w as i32 - sw as i32) / 2) as i32;
            let dy = ((tile_h as i32 - sh as i32) / 2) as i32;
            let x = base_x as i32 + dx;
            let y = base_y as i32 + dy;
            canvas.set_draw_color(to_color(theme.color("button_selected")));
            let pad = 2i32;
            let _ = canvas.fill_rect(sdl2::rect::Rect::new(
                x - pad,
                y - pad,
                sw + (pad * 2) as u32,
                sh + (pad * 2) as u32,
            ));
            canvas.set_draw_color(to_color(theme.color("border_selected")));
            let _ = canvas.draw_rect(sdl2::rect::Rect::new(
                x - pad,
                y - pad,
                sw + (pad * 2) as u32,
                sh + (pad * 2) as u32,
            ));
            // Box-art: secili tile ile birlikte olceklenen rect'e blit (yoksa fallback kutu kalir).
            let _ = art.blit(canvas, tc, &icon_path, sdl2::rect::Rect::new(x, y, sw, sh));
            let _ = crate::text::draw_text_centered(canvas, tc, &p.name, theme.color("neon"), sdl2::rect::Rect::new(x, y, sw, sh), 12, font_scale);

        } else {
            canvas.set_draw_color(to_color(theme.color("button_idle")));
            let _ = canvas.fill_rect(sdl2::rect::Rect::new(base_x as i32, base_y as i32, tile_w, tile_h));
            canvas.set_draw_color(to_color(theme.color("neon")));
            let _ = canvas.draw_rect(sdl2::rect::Rect::new(base_x as i32, base_y as i32, tile_w, tile_h));
            let _ = art.blit(canvas, tc, &icon_path, sdl2::rect::Rect::new(base_x as i32, base_y as i32, tile_w, tile_h));
            let _ = crate::text::draw_text_centered(canvas, tc, &p.name, theme.color("neon"), sdl2::rect::Rect::new(base_x as i32, base_y as i32, tile_w, tile_h), 11, font_scale);

        }
    }
}

/// Faz 4: oyun listesi (seçili platformun oyunları). Seçili satır `border_selected`.
fn draw_game_list(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    if screen.games.is_empty() {
        let bw = ((w as i32) * 60 / 100).max(40) as u32;
        let bh: u32 = 48;
        let x = ((w as i32 - bw as i32) / 2).max(0) as i32;
        let y = (h as i32 / 2).max(0) as i32;
        canvas.set_draw_color(to_color(theme.color("button_idle")));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bw, bh));
        let _ = crate::text::draw_text_centered(canvas, tc, "oyun yok", theme.color("neon"), sdl2::rect::Rect::new(x, y, bw, bh), 12, font_scale);

        return;
    }
    let row_h: u32 = 36;
    let gap: u32 = 4;
    let margin: u32 = 40;
    let avail_h = h.saturating_sub(margin * 2 + 40);
    let visible = (avail_h / (row_h + gap)) as usize;
    let start = screen.selected_game.saturating_sub(visible / 2).min(screen.games.len().saturating_sub(visible));
    let end = (start + visible).min(screen.games.len());
    let mut y = margin as i32 + 20;
    for (idx, g) in screen.games[start..end].iter().enumerate() {
        let abs_idx = start + idx;
        let is_sel = abs_idx == screen.selected_game;
        let row_color = if is_sel {
            theme.color("button_selected")
        } else {
            theme.color("button_idle")
        };
        let border = if is_sel {
            theme.color("border_selected")
        } else {
            theme.color("border")
        };
        canvas.set_draw_color(to_color(row_color));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(margin as i32, y, w - margin * 2, row_h));
        canvas.set_draw_color(to_color(border));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(margin as i32, y, w - margin * 2, row_h));
        if let Some(p) = screen.progress.get(&g.url) {
            if let Some(pct) = p.get("progress").and_then(|v| v.as_f64()) {
                let fill_w = ((w - margin * 2) as f64 * (pct / 100.0).clamp(0.0, 1.0)) as u32;
                if fill_w > 0 {
                    canvas.set_draw_color(to_color(theme.color("neon")));
                    let _ = canvas.fill_rect(sdl2::rect::Rect::new(margin as i32, y, fill_w, row_h));
                }
            }
        }
        // stale ttf guard kaldırıldı — fontdue dahili fallback kullanıyor
            let label = format!("{}  {}", g.name, g.size);
            let truncated = if label.chars().count() > 70 { label.chars().take(67).collect::<String>() + "..." } else { label };
            let text_color = if is_sel { theme.color("neon") } else { theme.color("neon") };
            let _ = crate::text::draw_text(canvas, tc, &truncated, text_color, margin as i32 + 8, y + 8, 11, font_scale);

        y += (row_h + gap) as i32;
    }
}

/// Footer — tuş atamaları (Python display/footer.py parity, TTF yok → renkli bar + border)
fn draw_footer(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
) {
    let bh: u32 = 36;
    let y = h.saturating_sub(bh) as i32;
    // Footer bar
    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, y, w, bh));
    canvas.set_draw_color(to_color(theme.color("border")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(0, y, w, bh));
    // Seçili menüye göre hint renkleri (metin yerine renk blokları — TTF ile sonra metin)
    let hints: &[&str] = match screen.menu {
        MenuState::PlatformGrid => &["↑↓←→ Gezin", "Enter Seç", "M Menü", "Esc Çık"],
        MenuState::GameList => &["↑↓ Seç", "Enter İndir", "Bksp Geri", "M Menü"],
        MenuState::Loading => &["R Retry", "Enter Çevrimdışı"],
        MenuState::Error(_) => &["R Retry", "Enter Çevrimdışı", "Esc Çık"],
        MenuState::Progress => &["Esc Geri"],
        MenuState::ConfirmExit => &["Enter Çık", "Esc İptal"],
    };
    // Her hint için küçük renkli kutu (TTF sonrası metin eklenecek)
    let mut x = 20;
    for hint in hints {
        let is_sel = hint.contains("Enter");
        let bg = if is_sel { theme.color("border_selected") } else { theme.color("button_selected") };
        canvas.set_draw_color(to_color(bg));
        // Hint genişliği metin uzunluğuna göre kabaca
        let hw = (hint.len() as u32 * 7 + 12).min(w.saturating_sub(40) / hints.len() as u32);
        if x + hw as i32 > w as i32 - 10 { break; }
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(x, y + 6, hw, bh - 12));
        canvas.set_draw_color(to_color(theme.color("neon")));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y + 6, hw, bh - 12));
        x += hw as i32 + 12;
    }
}

/// Faz 4: progress ekranı — seçili oyunun indirme ilerlemesi (SSE progress map).
fn draw_progress_screen(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    screen: &TvuiScreen,
    (w, h): (u32, u32),
    tc: &TextureCreator<WindowContext>,
    font_scale: f32,
) {
    let bar_w = ((w as i32) * 70 / 100).max(40) as u32;
    let bar_h: u32 = 28;
    let x = ((w as i32 - bar_w as i32) / 2).max(0) as i32;
    let y = (h as i32 / 2).max(0) as i32;
    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
    if let Some(g) = screen.games.get(screen.selected_game) {
        if let Some(p) = screen.progress.get(&g.url) {
            let pct = p.get("progress").and_then(|v| v.as_f64()).unwrap_or(0.0).clamp(0.0, 100.0) as f32 / 100.0;
            let fill_w = (bar_w as f32 * pct) as u32;
            if fill_w > 0 {
                canvas.set_draw_color(to_color(theme.color("neon")));
                let _ = canvas.fill_rect(sdl2::rect::Rect::new(x, y, fill_w, bar_h));
            }
            // stale ttf guard kaldırıldı — fontdue dahili fallback kullanıyor
                let pct_txt = format!("{}% - {}", (pct * 100.0) as i32, g.name.chars().take(40).collect::<String>());
                let _ = crate::text::draw_text_centered(canvas, tc, &pct_txt, theme.color("neon"), sdl2::rect::Rect::new(x, y + bar_h as i32 + 8, bar_w, 24), 12, font_scale);

            return;
        }
    }
    canvas.set_draw_color(to_color(theme.color("neon")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
    let _ = crate::text::draw_text_centered(canvas, tc, "indirme bekleniyor...", theme.color("neon"), sdl2::rect::Rect::new(x, y + bar_h as i32 + 8, bar_w, 24), 12, font_scale);

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
    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx, by, bw, total_h));
    canvas.set_draw_color(to_color(theme.color("border")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(bx, by, bw, total_h));
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
        canvas.set_draw_color(to_color(bg));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx + 10, y, bw - 20, bh_each));
        canvas.set_draw_color(to_color(border));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(bx + 10, y, bw - 20, bh_each));
        let _ = crate::text::draw_text_centered(canvas, tc, label, theme.color("neon"), sdl2::rect::Rect::new(bx + 10, y, bw - 20, bh_each), 12, font_scale);

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
        "ready" => "success",
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
                    // Nav/page/Back/Menu → state reducer (Faz 3+4+012i)
                    let nav_key = match kc {
                        Keycode::Up => Some(UiKey::NavUp),
                        Keycode::Down => Some(UiKey::NavDown),
                        Keycode::Left => Some(UiKey::NavLeft),
                        Keycode::Right => Some(UiKey::NavRight),
                        Keycode::PageUp => Some(UiKey::PageUp),
                        Keycode::PageDown => Some(UiKey::PageDown),
                        Keycode::Backspace => Some(UiKey::Back),
                        Keycode::M => Some(UiKey::Menu),
                        Keycode::Return | Keycode::KpEnter => Some(UiKey::Confirm),
                        _ => None,
                    };
                    if let Some(k) = nav_key {
                        let prev_menu = screen.menu.clone();
                        if let Some(action) = crate::state::reduce(&mut screen, k, now) {
                            apply_ui_action(state, action);
                        }
                        // Faz 4: PlatformGrid→GameList geçişinde oyunları çek
                        if matches!(prev_menu, MenuState::PlatformGrid)
                            && matches!(screen.menu, MenuState::GameList)
                            && screen.games.is_empty()
                        {
                            let plat = screen
                                .platforms
                                .get(screen.selected_platform)
                                .map(|p| p.folder.clone())
                                .unwrap_or_default();
                            if !plat.is_empty() {
                                let st = Arc::clone(state);
                                std::thread::spawn(move || {
                                    let games = crate::net::fetch_games(tvui_lock(&st).port, &plat);
                                    tvui_lock(&st).games = games;
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
                MenuState::Progress => draw_progress_screen(&mut canvas, theme, &screen, dims, &texture_creator, font_scale),
            }
            draw_footer(&mut canvas, theme, &screen, dims);
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
