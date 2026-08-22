//! TASK-012g — SDL2 native shell (temel: tam ekran init + arka plan paleti çizimi).
//!
//! `tvui.py` + `display/*` pygame `draw_*`'ları TASK-012h ve sonrasında SDL2
//! primitives'e portlanır. Bu modül yalnızca shell'i kurar ve `theme.json`
//! paletiyle arka plan gradyanını çizer (tema yüklendi kanıtı: `fond_lignes` çerçeve).

use std::time::Duration;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::video::Window;

use crate::net::SharedTvuiState;
use crate::theme::Theme;

fn to_color((r, g, b, a): (u8, u8, u8, u8)) -> Color {
    Color::RGBA(r, g, b, a)
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

/// Seçili arka plan preset'ini dikey gradyan olarak çizer (top → bottom).
fn draw_background(canvas: &mut Canvas<Window>, theme: &Theme, preset: &str) -> (u32, u32) {
    let (top, bottom) = theme.background(preset);
    let (w, h) = match canvas.output_size() {
        Ok((w, h)) if w > 0 && h > 0 => (w, h),
        _ => (1280, 720),
    };
    for y in 0..h {
        let t = if h <= 1 { 0.0 } else { y as f32 / (h - 1) as f32 };
        let r = lerp(top.0, bottom.0, t);
        let g = lerp(top.1, bottom.1, t);
        let b = lerp(top.2, bottom.2, t);
        canvas.set_draw_color(Color::RGB(r, g, b));
        let _ = canvas.draw_line((0, y as i32), (w as i32, y as i32));
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
fn draw_loading(canvas: &mut Canvas<Window>, theme: &Theme, state: &SharedTvuiState, (w, h): (u32, u32)) {
    let (pct, error) = {
        let s = state.lock().unwrap();
        (s.pct.clamp(0, 100) as f32 / 100.0, s.error.clone())
    };
    let bar_w = ((w as i32) * 60 / 100).max(40) as u32;
    let bar_h: u32 = 24;
    let x = ((w as i32 - bar_w as i32) / 2).max(0) as i32;
    let y = (h as i32 / 2).max(0) as i32;
    // Çerçeve (button_idle rengi).
    canvas.set_draw_color(to_color(theme.color("button_idle")));
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y, bar_w, bar_h));
    // Dolum (neon rengi).
    let fill_w = (bar_w as f32 * pct) as i32;
    if fill_w > 0 {
        canvas.set_draw_color(to_color(theme.color("neon")));
        let _ = canvas.fill_rect(sdl2::rect::Rect::new(x, y, fill_w as u32, bar_h));
    }
    if error.is_some() {
        // Hata: bar altına kırmızı çerçeve (metin yok — font yükleme henüz yok).
        canvas.set_draw_color(to_color(theme.color("error_text")));
        let _ = canvas.draw_rect(sdl2::rect::Rect::new(x, y + bar_h as i32 + 8, bar_w, 4));
    }
}

/// Native SDL2 TVUI shell'ini başlatır (tam ekran 10-foot). `Esc` / pencere
/// kapatma ile çıkılır. Bloklayıcıdır; manager-bin ayrı thread'de çağırır.
/// `state`: SSE `catalog_update` ilerlemesini çizen loading bar'ının kaynağı.
pub fn run_native_shell(theme: &Theme, state: &SharedTvuiState) -> Result<(), String> {
    let sdl = sdl2::init().map_err(|e| format!("SDL2 init: {e}"))?;
    let video = sdl.video().map_err(|e| format!("SDL2 video: {e}"))?;
    let window = video
        .window("RGSX", 1280, 720)
        .position_centered()
        .fullscreen()
        .build()
        .map_err(|e| format!("SDL2 pencere: {e}"))?;
    let mut canvas = window
        .into_canvas()
        .accelerated()
        .build()
        .map_err(|e| format!("SDL2 canvas: {e}"))?;
    let mut event_pump = sdl.event_pump().map_err(|e| format!("SDL2 event: {e}"))?;

    let preset = std::env::var("RGSX_TVUI_BG").unwrap_or_else(|_| "default".into());

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }
        let dims = draw_background(&mut canvas, theme, &preset);
        draw_loading(&mut canvas, theme, state, dims);
        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
