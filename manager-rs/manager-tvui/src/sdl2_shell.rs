//! TASK-012g — SDL2 native shell (temel: tam ekran init + arka plan paleti çizimi).
//!
//! `tvui.py` + `display/*` pygame `draw_*`'ları TASK-012h ve sonrasında SDL2
//! primitives'e portlanır. Bu modül yalnızca shell'i kurar ve `theme.json`
//! paletiyle arka plan gradyanını çizer (tema yüklendi kanıtı: `fond_lignes` çerçeve).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

use crate::net::{
    apply_ui_action, expire_stale_restart_at, tvui_lock, ui_decision, SharedTvuiState, UiKey,
};
use crate::theme::Theme;

fn to_color((r, g, b, a): (u8, u8, u8, u8)) -> Color {
    Color::RGBA(r, g, b, a)
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
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
    preset: &str,
) -> (u32, u32) {
    let (w, h) = match canvas.output_size() {
        Ok((w, h)) if w > 0 && h > 0 => (w, h),
        _ => (1280, 720),
    };
    let (top, bottom) = theme.background(preset);
    let stale = !matches!(cache, Some((cw, ch, _)) if *cw == w && *ch == h);
    if stale {
        let mut pixels: Vec<u8> = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            let t = if h <= 1 { 0.0 } else { y as f32 / (h - 1) as f32 };
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
                let t = if h <= 1 { 0.0 } else { y as f32 / (h - 1) as f32 };
                canvas.set_draw_color(Color::RGB(
                    lerp(top.0, bottom.0, t),
                    lerp(top.1, bottom.1, t),
                    lerp(top.2, bottom.2, t),
                ));
                let _ = canvas.draw_line((0, y as i32), (w as i32, y as i32));
            }
        }
    }
    // Tema paleti yüklendi kanıtı: `fond_lignes` rengiyle ince çerçeve.
    canvas.set_draw_color(to_color(theme.color("fond_lignes")));
    let (fw, fh) = (w.saturating_sub(40), h.saturating_sub(40));
    if fw > 0 && fh > 0 {
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(20, 20, fw, fh));
    }
    (w, h)
}

/// Açılış loading bar'ı: SSE `catalog_update` ilerlemesini `state`'ten okur.
/// `ready` oluncaya kadar (ya da hata varsa) ekranın ortasında çubuk çizer.
/// Hata varsa belirgin kırmızı çerçeve çizer (metin yok — TTF erte).
fn draw_loading(canvas: &mut Canvas<Window>, theme: &Theme, state: &SharedTvuiState, (w, h): (u32, u32)) {
    let (pct, error) = {
        let s = tvui_lock(state);
        (s.pct.clamp(0, 100) as f32 / 100.0, s.error.clone())
    };
    let bar_w = ((w as i32) * 60 / 100).max(40) as u32;
    let bar_h: u32 = 24;
    let x = ((w as i32 - bar_w as i32) / 2).max(0) as i32;
    let y = (h as i32 / 2).max(0) as i32;

    if error.is_some() {
        // Hata ekranı: üstte tam genişlik kırmızı şerit + orta çerçeve.
        canvas.set_draw_color(to_color(theme.color("error_text")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, (y - 40).max(0) as i32, w, 6));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
        return;
    }

    // Normal ilerleme: çerçeve (button_idle) + dolum (neon).
    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
    let fill_w = (bar_w as f32 * pct) as i32;
    if fill_w > 0 {
        canvas.set_draw_color(to_color(theme.color("neon")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(x, y, fill_w as u32, bar_h));
    }
}

/// `ready` sonrası platform grid'i: `/api/platforms`'tan gelen `state.platforms`
/// listesini tile olarak dizer (navigasyon/selection → sonraki faz). Metin etiketi
/// SDL2_ttf link ortamı hazır olunca eklenecek; şimdilik tile'lar veri güdümlü.
fn draw_grid(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    (w, _h): (u32, u32),
) {
    let (platforms, offline) = {
        let s = tvui_lock(state);
        (s.platforms.clone(), s.offline)
    };
    // Çevrimdışı mod: üstte kırmızı şerit (metin yok — TTF erte).
    if offline {
        canvas.set_draw_color(to_color(theme.color("error_text")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, 6));
    }
    if platforms.is_empty() {
        return;
    }
    let cols: u32 = 6;
    let gap: u32 = 16;
    let margin: u32 = 40;
    let avail_w = w.saturating_sub(margin * 2);
    let tile_w = (avail_w.saturating_sub(gap * (cols - 1))) / cols;
    let tile_h = tile_w * 3 / 4;
    for (i, _p) in platforms.iter().enumerate() {
        let col = (i as u32) % cols;
        let row = (i as u32) / cols;
        let x = margin + col * (tile_w + gap);
        let y = margin + row * (tile_h + gap);
        // Dolgu + neon çerçeve (her tile = bir gerçek platform).
        canvas.set_draw_color(to_color(theme.color("button_idle")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(
            x as i32,
            y as i32,
            tile_w,
            tile_h,
        ));
        canvas.set_draw_color(to_color(theme.color("neon")));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(
            x as i32,
            y as i32,
            tile_w,
            tile_h,
        ));
    }
}

/// TASK-012m Faz 5 — self-update banner (metin yok; ttf erte). Aşamaya göre renk:
/// `available`=warning_text (turuncu — bulgu 10 fix), `downloading`=neon (mavi,
/// iç dolgu=percent), `ready`=success (yeşil), `failed`=error_text (kırmızı).
fn draw_update_banner(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    state: &SharedTvuiState,
    (w, _h): (u32, u32),
) {
    let (avail, stage, pct) = {
        let s = tvui_lock(state);
        (s.update_available.clone(), s.update_stage.clone(), s.update_pct)
    };
    let Some(ver) = avail else {
        return;
    };
    let stage = stage.unwrap_or_else(|| "available".to_string());
    let color_key = match stage.as_str() {
        "ready" => "success",
        "downloading" => "neon",
        "failed" => "error_text",
        // Bulgu 10: 'available' turuncu — kırmızı yalnızca gerçek hataya kalsın.
        _ => "warning_text",
    };
    let bw = ((w as i32) * 60 / 100).max(40) as u32;
    let bx = ((w as i32 - bw as i32) / 2).max(0) as i32;
    let by = 8i32;
    let bh: u32 = 28;
    let color = to_color(theme.color(color_key));
    // Çerçeve.
    canvas.set_draw_color(color);
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(bx, by, bw, bh));
    // `downloading` aşamasında iç dolgu = ilerleme yüzdesi.
    if stage == "downloading" {
        let fill_w = ((bw as u64 * pct as u64 / 100) as u32).max(1).min(bw);
        canvas.set_draw_color(color);
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx, by, fill_w, bh));
    }
    // `ready` aşamasında yanıp sönen iç dolgu (uygula hazır).
    if stage == "ready" {
        canvas.set_draw_color(color);
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(bx, by, bw, bh));
    }
    let _ = ver;
}

/// TASK-012m Faz 5 — apply sonrası "Yeniden başlatılıyor…" tam ekran overlay'i
/// (metin yok; ttf erte — yalnız ayrı bir renk katmanı).
fn draw_restart_screen(
    canvas: &mut Canvas<Window>,
    theme: &Theme,
    (w, h): (u32, u32),
) {
    let c = to_color(theme.color("neon"));
    canvas.set_draw_color(c);
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(0, 0, w, h));
    // İçeride koyu bir dikdörtgen (karartma) — görsel vurgu.
    let _ = canvas.fill_rect(sdl2::rect::Rect::new(
        (w as i32 / 4).max(0),
        (h as i32 / 4).max(0),
        (w / 2).max(1),
        (h / 2).max(1),
    ));
}

/// Native SDL2 TVUI shell'ini başlatır (tam ekran 10-foot). `Esc` / pencere
/// kapatma / gamepad `back` ile çıkılır. Bloklayıcıdır; manager-bin ayrı
/// thread'de çağırır. `state`: SSE `catalog_update` ilerlemesini çizen loading
/// bar'ının kaynağı. `shutdown`: gamepad `back` (SSE) buraya yazılır.
pub fn run_native_shell(
    theme: &Theme,
    state: &SharedTvuiState,
    shutdown: &AtomicBool,
) -> Result<(), String> {
    let sdl = sdl2::init().map_err(|e| format!("SDL2 init: {e}"))?;
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
    let vsync = canvas.info().flags
        & sdl2::sys::SDL_RendererFlags::SDL_RENDERER_PRESENTVSYNC as u32
        != 0;
    let texture_creator = canvas.texture_creator();
    let mut bg_cache: Option<(u32, u32, Texture)> = None;
    let mut event_pump = sdl.event_pump().map_err(|e| format!("SDL2 event: {e}"))?;

    let preset = std::env::var("RGSX_TVUI_BG").unwrap_or_else(|_| "default".into());

    'running: loop {
        if shutdown.load(Ordering::Relaxed) {
            break 'running; // Faz C bulgu 9: gamepad back.
        }
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                // TASK-012-gap-01 Faz B (bulgu 15): kararlar SDL'siz `ui_decision`'da,
                // HTTP arka planda (`apply_ui_action`) — event loop asla bloklanmaz.
                Event::KeyDown { keycode: Some(kc), .. } => {
                    let key = match kc {
                        Keycode::R => Some(UiKey::Retry),
                        Keycode::Return | Keycode::KpEnter => Some(UiKey::Confirm),
                        Keycode::C => Some(UiKey::CancelUpdate),
                        _ => None,
                    };
                    if let Some(key) = key {
                        let action = {
                            let s = tvui_lock(state);
                            ui_decision(&s, key)
                        };
                        if let Some(action) = action {
                            apply_ui_action(state, action);
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
        let dims = draw_background(
            &mut canvas,
            &texture_creator,
            &mut bg_cache,
            theme,
            &preset,
        );
        draw_update_banner(&mut canvas, theme, state, dims);
        // Yeniden başlatma ekranı (apply sonrası) — grid/loading yerine tam ekran.
        let restarting = tvui_lock(state).update_restarting;
        if restarting {
            draw_restart_screen(&mut canvas, theme, dims);
        } else {
            // Loading → ready/offline → platform_grid geçişi (012h omurgası).
            let show_grid = {
                let s = tvui_lock(state);
                s.ready || s.offline
            };
            if show_grid {
                draw_grid(&mut canvas, theme, state, dims);
            } else {
                draw_loading(&mut canvas, theme, state, dims);
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
