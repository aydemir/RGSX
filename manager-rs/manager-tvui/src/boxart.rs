//! TASK-012-gap-05 — box-art texture cache (SDL tarafı).
//!
//! `render::BoxArtCache` SDL-free karar katmanıdır (yol + ham-bayt LRU).
//! Bu modül SDL tarafıdır: `image` ile decode → RGBA → `Texture`,
//! `HashMap` + LRU sıra, cap 64. Dosya yok/bozuksa `false` döner —
//! çağıran mevcut fallback renk kutuyu korur (doğruluk > zarafet).

use std::collections::HashMap;

use sdl2::pixels::PixelFormatEnum;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, Texture, TextureCreator};
use sdl2::video::{Window, WindowContext};

/// Cap: aynı anda en fazla bu kadar texture canlı tutulur.
pub const MAX_TEXTURES: usize = 64;

/// Ham dosya baytını RGBA pixellere çevirir (SDL gerektirmez, test edilir).
/// Dönüş: (rgba bayt, genişlik, yükseklik).
pub fn decode_file_bytes(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    let rgba = img.to_rgba8();
    let (w, h) = (rgba.width(), rgba.height());
    if w == 0 || h == 0 {
        return None;
    }
    Some((rgba.into_raw(), w, h))
}

pub struct SdlBoxArtCache<'a> {
    textures: HashMap<String, Texture<'a>>,
    order: Vec<String>, // LRU: en eski başta
    max_entries: usize,
    icons_path: String,
    pub hits: usize,
    pub misses: usize,
}

impl<'a> SdlBoxArtCache<'a> {
    pub fn new(max_entries: usize, icons_path: String) -> Self {
        Self {
            textures: HashMap::new(),
            order: Vec::new(),
            max_entries: max_entries.max(1),
            icons_path,
            hits: 0,
            misses: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    /// `icons.path` değiştiyse önbelleği boşalt (stale texture çizilmesin).
    /// Değişiklikte `true` döner.
    pub fn sync_icons_path(&mut self, current: &str) -> bool {
        if self.icons_path != current {
            self.icons_path = current.to_string();
            self.textures.clear();
            self.order.clear();
            true
        } else {
            false
        }
    }

    fn touch(&mut self, key: &str) {
        if let Some(pos) = self.order.iter().position(|k| k == key) {
            let k = self.order.remove(pos);
            self.order.push(k);
        }
    }

    fn evict_if_full(&mut self) {
        while self.textures.len() >= self.max_entries {
            if let Some(old) = self.order.first().cloned() {
                self.order.remove(0);
                self.textures.remove(&old);
            } else {
                break;
            }
        }
    }

    /// Ham RGBA pixeli texture'a çevirip önbelleğe koy (dosya okumasız yol).
    pub fn insert_rgba(
        &mut self,
        tc: &'a TextureCreator<WindowContext>,
        key: String,
        rgba: &[u8],
        w: u32,
        h: u32,
    ) -> bool {
        if w == 0 || h == 0 || rgba.len() != (w * h * 4) as usize {
            return false;
        }
        if self.textures.contains_key(&key) {
            self.touch(&key);
            return true;
        }
        self.evict_if_full();
        let mut tex = match tc.create_texture_static(PixelFormatEnum::RGBA32, w, h) {
            Ok(t) => t,
            Err(_) => return false,
        };
        if tex.update(None, rgba, (w * 4) as usize).is_err() {
            return false;
        }
        tex.set_blend_mode(sdl2::render::BlendMode::Blend);
        self.order.push(key.clone());
        self.textures.insert(key, tex);
        true
    }

    /// `icon_path`'i önbellekten ya da diskten yükleyip `dst`'ye blit eder.
    /// Başarısızlıkta `false` (çağıran fallback kutuyu korur).
    pub fn blit(
        &mut self,
        canvas: &mut Canvas<Window>,
        tc: &'a TextureCreator<WindowContext>,
        icon_path: &str,
        dst: Rect,
    ) -> bool {
        if dst.width() == 0 || dst.height() == 0 {
            return false;
        }
        if self.textures.contains_key(icon_path) {
            self.hits += 1;
            self.touch(icon_path);
        } else {
            self.misses += 1;
            let bytes = match std::fs::read(icon_path) {
                Ok(b) => b,
                Err(_) => return false,
            };
            let (rgba, w, h) = match decode_file_bytes(&bytes) {
                Some(v) => v,
                None => return false,
            };
            if !self.insert_rgba(tc, icon_path.to_string(), &rgba, w, h) {
                return false;
            }
        }
        match self.textures.get(icon_path) {
            Some(tex) => canvas.copy(tex, None, Some(dst)).is_ok(),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2x2 kırmızı PNG'yi memory'de üretir (gömülü asset yok).
    fn red_png_bytes() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 0, 0, 255]));
        let mut buf = Vec::new();
        let enc = image::codecs::png::PngEncoder::new(&mut buf);
        image::ImageEncoder::write_image(
            enc,
            img.as_raw(),
            2,
            2,
            image::ExtendedColorType::Rgba8,
        )
        .expect("png encode");
        buf
    }

    #[test]
    fn decode_png_roundtrip() {
        let bytes = red_png_bytes();
        let (rgba, w, h) = decode_file_bytes(&bytes).expect("decode");
        assert_eq!((w, h), (2, 2));
        assert_eq!(rgba.len(), 2 * 2 * 4);
        assert_eq!(&rgba[0..4], &[255, 0, 0, 255]);
    }

    #[test]
    fn decode_garbage_returns_none() {
        assert!(decode_file_bytes(b"bu bir png degil").is_none());
        assert!(decode_file_bytes(&[]).is_none());
    }

    #[test]
    fn cache_cap_evicts_oldest_with_dummy_video() {
        std::env::set_var("SDL_VIDEODRIVER", "dummy");
        let sdl = sdl2::init().expect("sdl init");
        let video = sdl.video().expect("video");
        let window = video.window("t", 64, 64).build().expect("window");
        let canvas = window.into_canvas().software().build().expect("canvas");
        let tc = canvas.texture_creator();
        let mut cache = SdlBoxArtCache::new(2, "icons".into());
        let px = vec![255u8; 4 * 4 * 4]; // 4x4 opak
        assert!(cache.insert_rgba(&tc, "a".into(), &px, 4, 4));
        assert!(cache.insert_rgba(&tc, "b".into(), &px, 4, 4));
        assert_eq!(cache.len(), 2);
        assert!(cache.insert_rgba(&tc, "c".into(), &px, 4, 4)); // a atılır
        assert!(cache.len() <= 2);
        assert!(!cache.textures.contains_key("a"));
        assert!(cache.textures.contains_key("c"));
        // icons.path değişimi invalidate eder
        assert!(cache.sync_icons_path("other"));
        assert_eq!(cache.len(), 0);
        assert!(!cache.sync_icons_path("other"));
    }
}
