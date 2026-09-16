//! TASK-012-gap-04 — TTF metin render yardimcisi (fontdue + ab_glyph fallback).
//!
//! `sdl2_ttf` bagimliligi sdl2-sys link catismasi yuzunden kaldirildi (sdl2 0.37 vs ttf 0.25).
//! Yerine pure-Rust `fontdue` ile rasterize edip SDL2 texture'a ceviriyoruz.
//! Font yoksa sessizce atlar.

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

const FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
];

/// `font_scale`'i metin boyutuna uygular (8–64 px clamp).
/// SDL nesnesi gerektirmez — `draw_text*` ile aynı formül.
pub fn scaled_size(base_size: u16, font_scale: f32) -> f32 {
    (base_size as f32 * font_scale).max(8.0).min(64.0)
}

fn load_font_bytes() -> Option<Vec<u8>> {
    for cand in FONT_CANDIDATES {
        if let Ok(b) = std::fs::read(cand) {
            return Some(b);
        }
    }
    None
}

/// Metni canvas uzerine cizer. Basarisiz olursa false.
pub fn draw_text(
    canvas: &mut Canvas<Window>,
    texture_creator: &TextureCreator<WindowContext>,
    text: &str,
    color: (u8, u8, u8, u8),
    x: i32,
    y: i32,
    base_size: u16,
    font_scale: f32,
) -> bool {
    if text.is_empty() {
        return false;
    }
    let size = scaled_size(base_size, font_scale);
    let font_bytes = match load_font_bytes() {
        Some(b) => b,
        None => return false,
    };
    let font = match fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()) {
        Ok(f) => f,
        Err(_) => return false,
    };
    // Measure total width
    let mut total_w: usize = 0;
    let mut max_h: usize = 0;
    for ch in text.chars() {
        let (metrics, _) = font.rasterize(ch, size);
        total_w += metrics.width;
        if metrics.height > max_h {
            max_h = metrics.height;
        }
        // advance a bit
        total_w += 1;
    }
    if total_w == 0 || max_h == 0 {
        return false;
    }
    // Create RGBA buffer
    let mut buffer = vec![0u8; total_w * max_h * 4];
    let mut cursor_x = 0usize;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col];
                if alpha == 0 {
                    continue;
                }
                let bx = cursor_x + col;
                let by = row;
                if bx >= total_w || by >= max_h {
                    continue;
                }
                let idx = (by * total_w + bx) * 4;
                buffer[idx] = color.0;
                buffer[idx + 1] = color.1;
                buffer[idx + 2] = color.2;
                buffer[idx + 3] = alpha;
            }
        }
        cursor_x += metrics.width + 1;
    }
    // Create texture from buffer
    let mut texture = match texture_creator.create_texture_static(PixelFormatEnum::RGBA32, total_w as u32, max_h as u32) {
        Ok(t) => t,
        Err(_) => return false,
    };
    if texture.update(None, &buffer, (total_w * 4) as usize).is_err() {
        return false;
    }
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    let dst = Rect::new(x, y, total_w as u32, max_h as u32);
    let _ = canvas.copy(&texture, None, Some(dst));
    true
}

pub fn draw_text_centered(
    canvas: &mut Canvas<Window>,
    texture_creator: &TextureCreator<WindowContext>,
    text: &str,
    color: (u8, u8, u8, u8),
    rect: Rect,
    base_size: u16,
    font_scale: f32,
) -> bool {
    if text.is_empty() {
        return false;
    }
    let size = scaled_size(base_size, font_scale);
    let font_bytes = match load_font_bytes() {
        Some(b) => b,
        None => return false,
    };
    let font = match fontdue::Font::from_bytes(font_bytes, fontdue::FontSettings::default()) {
        Ok(f) => f,
        Err(_) => return false,
    };
    let mut total_w: usize = 0;
    let mut max_h: usize = 0;
    for ch in text.chars() {
        let (metrics, _) = font.rasterize(ch, size);
        total_w += metrics.width + 1;
        if metrics.height > max_h {
            max_h = metrics.height;
        }
    }
    if total_w == 0 || max_h == 0 {
        return false;
    }
    let mut buffer = vec![0u8; total_w * max_h * 4];
    let mut cursor_x = 0usize;
    for ch in text.chars() {
        let (metrics, bitmap) = font.rasterize(ch, size);
        for row in 0..metrics.height {
            for col in 0..metrics.width {
                let alpha = bitmap[row * metrics.width + col];
                if alpha == 0 {
                    continue;
                }
                let bx = cursor_x + col;
                let by = row;
                if bx >= total_w || by >= max_h {
                    continue;
                }
                let idx = (by * total_w + bx) * 4;
                buffer[idx] = color.0;
                buffer[idx + 1] = color.1;
                buffer[idx + 2] = color.2;
                buffer[idx + 3] = alpha;
            }
        }
        cursor_x += metrics.width + 1;
    }
    let mut texture = match texture_creator.create_texture_static(PixelFormatEnum::RGBA32, total_w as u32, max_h as u32) {
        Ok(t) => t,
        Err(_) => return false,
    };
    if texture.update(None, &buffer, (total_w * 4) as usize).is_err() {
        return false;
    }
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    let x = rect.x() + ((rect.width() as i32 - total_w as i32) / 2).max(0);
    let y = rect.y() + ((rect.height() as i32 - max_h as i32) / 2).max(0);
    let dst = Rect::new(x, y, total_w as u32, max_h as u32);
    let _ = canvas.copy(&texture, None, Some(dst));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_scale_clamping() {
        assert_eq!(scaled_size(16, 0.1), 8.0);
        assert_eq!(scaled_size(16, 10.0), 64.0);
    }

    #[test]
    fn font_scale_applies_proportionally() {
        assert_eq!(scaled_size(12, 1.0), 12.0);
        assert_eq!(scaled_size(12, 1.5), 18.0);
        assert_eq!(scaled_size(11, 0.5), 8.0); // alt clamp
    }
}
