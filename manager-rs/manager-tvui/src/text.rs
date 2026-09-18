//! TASK-012-gap-04 — TTF metin render yardimcisi (fontdue + ab_glyph fallback).
//!
//! `sdl2_ttf` bagimliligi sdl2-sys link catismasi yuzunden kaldirildi (sdl2 0.37 vs ttf 0.25).
//! Yerine pure-Rust `fontdue` ile rasterize edip SDL2 texture'a ceviriyoruz.
//! Font yoksa sessizce atlar.

use std::path::PathBuf;
use std::sync::OnceLock;

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator};
use sdl2::video::{Window, WindowContext};

/// Linux font konumları (Batocera/proot hedefi; eskiden tek kaynaktı).
const LINUX_FONT_CANDIDATES: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
];

/// Windows sistem fontları (Türkçe glif kapsama: Arial + Segoe UI).
const WINDOWS_FONT_CANDIDATES: &[&str] = &[
    "C:/Windows/Fonts/arial.ttf",
    "C:/Windows/Fonts/segoeui.ttf",
    "C:/Windows/Fonts/DejaVuSans.ttf",
];

/// Gömülü/asset font dosya adları (`assets/fonts/` altında aranır).
const ASSET_FONT_NAMES: &[&str] = &["Pixel-UniCode.ttf", "DejaVuSans.ttf"];

/// Font arama sırası (SDL'siz, test edilebilir):
/// 1. `RGSX_TVUI_FONT` env (explicit override)
/// 2. asset fontlar: CWD-relative, exe-relative, `CARGO_MANIFEST_DIR`-relative (dev)
/// 3. platform sistem fontları (Windows / Linux)
pub fn font_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(p) = std::env::var("RGSX_TVUI_FONT") {
        if !p.trim().is_empty() {
            out.push(PathBuf::from(p));
        }
    }
    for name in ASSET_FONT_NAMES {
        out.push(PathBuf::from("assets/fonts").join(name)); // CWD (cargo test / dev)
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                out.push(dir.join("assets/fonts").join(name)); // deploy yanı
            }
        }
        out.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/fonts").join(name));
    }
    #[cfg(windows)]
    for cand in WINDOWS_FONT_CANDIDATES {
        out.push(PathBuf::from(cand));
    }
    #[cfg(unix)]
    for cand in LINUX_FONT_CANDIDATES {
        out.push(PathBuf::from(cand));
    }
    out
}

/// `font_scale`'i metin boyutuna uygular (8–64 px clamp).
/// SDL nesnesi gerektirmez — `draw_text*` ile aynı formül.
pub fn scaled_size(base_size: u16, font_scale: f32) -> f32 {
    (base_size as f32 * font_scale).max(8.0).min(64.0)
}

/// İlk bulunan fontun baytı (tek FS okuma; `OnceLock` ile süreç boyu cache).
/// Yoksa `None` → çağıran sessizce atlar (eski davranış korunur).
fn load_font_bytes() -> Option<Vec<u8>> {
    static CACHE: OnceLock<Option<Vec<u8>>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            for cand in font_candidates() {
                if let Ok(b) = std::fs::read(&cand) {
                    if !b.is_empty() {
                        return Some(b);
                    }
                }
            }
            None
        })
        .clone()
}

/// Parse edilmiş font (pahalı `from_bytes` kare başına DEĞİL, bir kez).
/// `fontdue::Font` `Send+Sync` ise `OnceLock`'ta taşınır.
fn load_font() -> Option<&'static fontdue::Font> {
    static CACHE: OnceLock<Option<fontdue::Font>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            load_font_bytes().and_then(|b| {
                fontdue::Font::from_bytes(b, fontdue::FontSettings::default()).ok()
            })
        })
        .as_ref()
}

/// Dizilmiş metin (RGBA tampon + boyut). Taban çizgisi hizalı:
/// her glif `ymin` bearing'ine göre düşeyde oturtulur (inişli `g/y/p`
/// sarkar, `A` ile aynı tabanda durur — upstream/pygame parity).
struct LaidText {
    buffer: Vec<u8>,
    w: usize,
    h: usize,
}

fn layout_text(
    font: &fontdue::Font,
    text: &str,
    size: f32,
    color: (u8, u8, u8, u8),
) -> Option<LaidText> {
    let adv = |m: &fontdue::Metrics| (m.advance_width.ceil() as usize).max(1);
    // 1. Rasterize + ölçüler (taban üstü = ymin+yükseklik).
    let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
    let mut total_w: usize = 0;
    let mut max_top: i32 = 0;
    let mut min_ymin: i32 = 0;
    let mut any_ink = false;
    for ch in text.chars() {
        let (m, bmp) = font.rasterize(ch, size);
        total_w += adv(&m);
        if m.width > 0 && m.height > 0 && !bmp.iter().all(|&a| a == 0) {
            any_ink = true;
            max_top = max_top.max(m.ymin + m.height as i32);
            min_ymin = min_ymin.min(m.ymin);
        }
        glyphs.push((m, bmp));
    }
    if total_w == 0 || !any_ink {
        return None;
    }
    // 2. Negatif sol yatak (italik taşma) kaydırılır.
    let shift_x = glyphs
        .iter()
        .map(|(m, _)| m.xmin)
        .min()
        .unwrap_or(0)
        .min(0)
        .unsigned_abs() as usize;
    total_w = total_w.saturating_add(shift_x);
    let total_h = (max_top - min_ymin).max(1) as usize;
    // 3. Blit (mürekkepsiz glifler yalnızca ilerletir).
    let mut buffer = vec![0u8; total_w * total_h * 4];
    let mut cursor_x = shift_x;
    for (m, bmp) in &glyphs {
        if m.width > 0 && m.height > 0 {
            let gx = cursor_x as i32 + m.xmin;
            let gy = max_top - (m.ymin + m.height as i32);
            for row in 0..m.height {
                for col in 0..m.width {
                    let alpha = bmp[row * m.width + col];
                    if alpha == 0 {
                        continue;
                    }
                    let bx = gx + col as i32;
                    let by = gy + row as i32;
                    if bx < 0 || by < 0 || bx >= total_w as i32 || by >= total_h as i32 {
                        continue;
                    }
                    let idx = (by as usize * total_w + bx as usize) * 4;
                    buffer[idx] = color.0;
                    buffer[idx + 1] = color.1;
                    buffer[idx + 2] = color.2;
                    buffer[idx + 3] = alpha;
                }
            }
        }
        cursor_x += adv(m);
    }
    Some(LaidText { buffer, w: total_w, h: total_h })
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
    let font = match load_font() {
        Some(f) => f,
        None => return false,
    };
    let laid = match layout_text(font, text, size, color) {
        Some(l) => l,
        None => return false,
    };
    // Create texture from buffer
    let mut texture = match texture_creator.create_texture_static(PixelFormatEnum::RGBA32, laid.w as u32, laid.h as u32) {
        Ok(t) => t,
        Err(_) => return false,
    };
    if texture.update(None, &laid.buffer, (laid.w * 4) as usize).is_err() {
        return false;
    }
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    let dst = Rect::new(x, y, laid.w as u32, laid.h as u32);
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
    let font = match load_font() {
        Some(f) => f,
        None => return false,
    };
    let laid = match layout_text(font, text, size, color) {
        Some(l) => l,
        None => return false,
    };
    let mut texture = match texture_creator.create_texture_static(PixelFormatEnum::RGBA32, laid.w as u32, laid.h as u32) {
        Ok(t) => t,
        Err(_) => return false,
    };
    if texture.update(None, &laid.buffer, (laid.w * 4) as usize).is_err() {
        return false;
    }
    texture.set_blend_mode(sdl2::render::BlendMode::Blend);
    let x = rect.x() + ((rect.width() as i32 - laid.w as i32) / 2).max(0);
    let y = rect.y() + ((rect.height() as i32 - laid.h as i32) / 2).max(0);
    let dst = Rect::new(x, y, laid.w as u32, laid.h as u32);
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

    #[test]
    fn space_has_positive_advance() {
        // Sözleşme bulgusu: boşluk bit eşlemsiz ama ilerlemeli, yoksa
        // "Page 1/13" → "Page1/13" diye bitişir.
        let bytes = load_font_bytes().expect("test asset fontu bulunmalı");
        let font = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()).unwrap();
        let (m, _) = font.rasterize(' ', 16.0);
        assert!(m.advance_width > 1.0, "boşluk ilerlemeli: {}", m.advance_width);
    }

    #[test]
    fn layout_baseline_descenders_extend_height() {
        // `g` inişli: "Ag" kutusu "A"dan yüksek olmalı (taban hizası korunur).
        let font = load_font().expect("test asset fontu bulunmalı");
        let a = layout_text(font, "A", 16.0, (255, 255, 255, 255)).unwrap();
        let ag = layout_text(font, "Ag", 16.0, (255, 255, 255, 255)).unwrap();
        assert!(ag.h > a.h, "A:{} Ag:{}", a.h, ag.h);
        // Boşluk genişletir ama mürekkep eklemez.
        let ab = layout_text(font, "ab", 16.0, (255, 255, 255, 255)).unwrap();
        let aspb = layout_text(font, "a b", 16.0, (255, 255, 255, 255)).unwrap();
        assert!(aspb.w > ab.w, "ab:{} 'a b':{}", ab.w, aspb.w);
        // Yalnız boşluk → None (çizilecek mürekkep yok).
        assert!(layout_text(font, "   ", 16.0, (255, 255, 255, 255)).is_none());
    }

    #[test]
    fn font_candidates_cover_platform() {
        let cands = font_candidates();
        assert!(!cands.is_empty());
        // Asset fontlar her platformda aranır (repo'ya gömülü DejaVuSans/Pixel-UniCode).
        assert!(cands.iter().any(|p| p.ends_with("DejaVuSans.ttf")));
        #[cfg(windows)]
        assert!(cands.iter().any(|p| p.ends_with("arial.ttf")));
        #[cfg(unix)]
        assert!(cands.iter().any(|p| p.ends_with("DejaVuSans.ttf")));
    }

    #[test]
    fn font_candidates_env_override_first() {
        let prev = std::env::var("RGSX_TVUI_FONT").ok();
        std::env::set_var("RGSX_TVUI_FONT", "/tmp/test-font.ttf");
        let cands = font_candidates();
        assert_eq!(cands[0], PathBuf::from("/tmp/test-font.ttf"));
        match prev {
            Some(v) => std::env::set_var("RGSX_TVUI_FONT", v),
            None => std::env::remove_var("RGSX_TVUI_FONT"),
        }
    }
}
