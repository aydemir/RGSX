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

use crate::theme::Theme;

fn to_color((r, g, b, a): (u8, u8, u8, u8)) -> Color {
    Color::RGBA(r, g, b, a)
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

/// Seçili arka plan preset'ini dikey gradyan olarak çizer (top → bottom).
fn draw_background(canvas: &mut Canvas<Window>, theme: &Theme, preset: &str) {
    let (top, bottom) = theme.background(preset);
    let (w, h) = canvas.output_size().unwrap_or((1280, 720));
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
    let _ = canvas.draw_rect(sdl2::rect::Rect::new(20, 20, w - 40, h - 40));
}

/// Native SDL2 TVUI shell'ini başlatır (tam ekran 10-foot). `Esc` / pencere
/// kapatma ile çıkılır. Bloklayıcıdır; manager-bin ayrı thread'de çağırır.
pub fn run_native_shell(theme: &Theme) -> Result<(), String> {
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
        draw_background(&mut canvas, theme, &preset);
        canvas.present();
        std::thread::sleep(Duration::from_millis(16));
    }
    Ok(())
}
