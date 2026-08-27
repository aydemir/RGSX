//! TASK-012h Faz 5 — transition + box-art cache (SDL'siz çekirdek).
//!
//! `transitions.py:draw_validation_transition` (scale 1.5→2.5, 1000ms, neon)
//! ve `display/grid.py` box-art yükleme önbelleği SDL'siz portlanır.
//! SDL yalnız piksel işi; karar ve hesap burada, test edilir.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Platform seçim transition'ı (scale + alpha).
/// `theme.json:transitions.platform_select` ile aynı parametreler.
#[derive(Debug, Clone)]
pub struct Transition {
    pub start: Instant,
    pub duration: Duration,
    pub scale_min: f32,
    pub scale_max: f32,
}

impl Transition {
    pub fn new(now: Instant, duration_ms: u64, scale_min: f32, scale_max: f32) -> Self {
        Self {
            start: now,
            duration: Duration::from_millis(duration_ms),
            scale_min,
            scale_max,
        }
    }

    /// `now` anındaki (scale, alpha). `None` → bitti, `Some` → devam.
    /// alpha 1.0 → 0.3 lineer düşer (neon fade).
    pub fn sample(&self, now: Instant) -> Option<(f32, f32)> {
        let elapsed = now.duration_since(self.start);
        if elapsed >= self.duration {
            return None;
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        // ease-out cubic
        let ease = 1.0 - (1.0 - t).powf(3.0);
        let scale = self.scale_min + (self.scale_max - self.scale_min) * ease;
        let alpha = 1.0 - 0.7 * ease; // 1.0 → 0.3
        Some((scale, alpha))
    }

    pub fn is_finished(&self, now: Instant) -> bool {
        now.duration_since(self.start) >= self.duration
    }
}

/// Box-art / ikon önbelleği — SDL texture değil, ham bayt + LRU (SDL'siz test).
/// Gerçek texture cache `sdl2_shell`'da `TextureCreator` ile tutulur; burası
/// karar katmanı (hangi dosyanın yükleneceği, boyut sınırı).
#[derive(Debug, Default)]
pub struct BoxArtCache {
    map: HashMap<String, Vec<u8>>,
    order: Vec<String>, // LRU: en eski başta
    max_entries: usize,
    pub hits: usize,
    pub misses: usize,
}

impl BoxArtCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: Vec::new(),
            max_entries: max_entries.max(1),
            hits: 0,
            misses: 0,
        }
    }

    /// `key` için önbellekte varsa `Some(&bytes)`, yoksa `None` (miss sayılır).
    pub fn get(&mut self, key: &str) -> Option<&Vec<u8>> {
        if self.map.contains_key(key) {
            self.hits += 1;
            // LRU: sona taşı
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                let k = self.order.remove(pos);
                self.order.push(k);
            }
            self.map.get(key)
        } else {
            self.misses += 1;
            None
        }
    }

    /// `key` → `bytes` koy; `max_entries` aşılırsa en eski atılır.
    pub fn insert(&mut self, key: String, bytes: Vec<u8>) {
        if self.map.contains_key(&key) {
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
        } else if self.map.len() >= self.max_entries {
            if let Some(old) = self.order.first().cloned() {
                self.order.remove(0);
                self.map.remove(&old);
            }
        }
        self.order.push(key.clone());
        self.map.insert(key, bytes);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn contains(&self, key: &str) -> bool {
        self.map.contains_key(key)
    }

    /// `platform_id` için ikon dosya yolunu çözer (theme.icons.path + set).
    pub fn icon_path_for(platform_id: &str, icons_path: &str) -> String {
        let base = icons_path.trim_end_matches('/');
        format!("{base}/{platform_id}.png")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn transition_samples_scale_alpha() {
        let start = Instant::now();
        let tr = Transition::new(start, 1000, 1.5, 2.5);
        // t=0 → scale_min
        let (s0, a0) = tr.sample(start).unwrap();
        assert!((s0 - 1.5).abs() < 0.01);
        assert!((a0 - 1.0).abs() < 0.01);
        // t=500ms → orta
        let mid = start + Duration::from_millis(500);
        let (sm, am) = tr.sample(mid).unwrap();
        assert!(sm > 1.5 && sm < 2.5);
        assert!(am < 1.0 && am > 0.3);
        // t=1000ms → bitti
        assert!(tr.sample(start + Duration::from_millis(1000)).is_none());
        assert!(tr.is_finished(start + Duration::from_millis(1000)));
    }

    #[test]
    fn transition_finishes_after_duration() {
        let start = Instant::now();
        let tr = Transition::new(start, 200, 1.0, 2.0);
        assert!(!tr.is_finished(start + Duration::from_millis(100)));
        assert!(tr.is_finished(start + Duration::from_millis(300)));
    }

    #[test]
    fn boxart_cache_lru() {
        let mut c = BoxArtCache::new(2);
        c.insert("a".into(), vec![1]);
        c.insert("b".into(), vec![2]);
        assert_eq!(c.len(), 2);
        c.insert("c".into(), vec![3]); // a atılır
        assert!(!c.contains("a"));
        assert!(c.contains("b"));
        assert!(c.contains("c"));
        // hit
        assert!(c.get("b").is_some());
        assert_eq!(c.hits, 1);
        // miss
        assert!(c.get("missing").is_none());
        assert_eq!(c.misses, 1);
    }

    #[test]
    fn boxart_icon_path() {
        assert_eq!(
            BoxArtCache::icon_path_for("nes", "assets/icons"),
            "assets/icons/nes.png"
        );
        assert_eq!(
            BoxArtCache::icon_path_for("snes", "assets/icons/"),
            "assets/icons/snes.png"
        );
    }

    #[test]
    fn boxart_cache_hit_moves_to_end() {
        let mut c = BoxArtCache::new(3);
        c.insert("a".into(), vec![1]);
        c.insert("b".into(), vec![2]);
        c.insert("c".into(), vec![3]);
        // order a,b,c → get a → order b,c,a
        c.get("a");
        c.insert("d".into(), vec![4]); // b atılır (en eski)
        assert!(!c.contains("b"));
        assert!(c.contains("a"));
    }
}
