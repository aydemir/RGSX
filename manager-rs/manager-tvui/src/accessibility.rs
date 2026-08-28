//! TASK-012k — Erişilebilirlik (font_scale / footer_font_scale / yüksek kontrast).
//!
//! `accessibility.py` (font_scale, footer_font_scale) + `display/fonts.py`
//! (font_scale_options, footer_font_scale_options) + `display/colors.py`
//! (yüksek kontrast paleti) SDL2'ye portlanır. Canlı uygulama; tema/state
//! düzeyi ölçek ve palet override.

use crate::theme::Theme;

/// `fonts.py` parity: genel metin ölçek seçenekleri.
pub const FONT_SCALE_OPTIONS: &[f32] = &[
    0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0,
];

/// `fonts.py` / `config.py` parity: footer ayrı ölçeklenir (19 değer, 0.7..2.5).
pub const FOOTER_FONT_SCALE_OPTIONS: &[f32] = &[
    0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.7, 1.8, 1.9, 2.0, 2.1, 2.2, 2.3, 2.4, 2.5,
];

/// Yüksek kontrast paleti — eski TVUI ile eşleşir, uygulanınca anında.
/// Siyah arka plan, sarı/beyaz vurgu, yüksek okunabilirlik.
pub fn high_contrast_palette() -> std::collections::HashMap<String, Vec<u8>> {
    let mut m = std::collections::HashMap::new();
    // Arka plan tamamen siyah
    m.insert("background_top".into(), vec![0, 0, 0]);
    m.insert("background_bottom".into(), vec![0, 0, 0]);
    // Metin saf beyaz, seçili sarı
    m.insert("text".into(), vec![255, 255, 255]);
    m.insert("text_selected".into(), vec![255, 255, 0]);
    m.insert("title_text".into(), vec![255, 255, 255]);
    // Butonlar siyah/beyaz kontrast
    m.insert("button_idle".into(), vec![0, 0, 0, 220]);
    m.insert("button_selected".into(), vec![255, 255, 0, 255]);
    m.insert("button_hover".into(), vec![255, 255, 255, 255]);
    // Vurgu / neon sarı
    m.insert("neon".into(), vec![255, 255, 0]);
    m.insert("fond_lignes".into(), vec![255, 255, 0]);
    m.insert("fond_image".into(), vec![30, 30, 30]);
    // Border beyaz/sarı
    m.insert("border".into(), vec![255, 255, 255]);
    m.insert("border_selected".into(), vec![255, 255, 0]);
    // Hata / başarı / uyarı canlı
    m.insert("error_text".into(), vec![255, 50, 50]);
    m.insert("success_text".into(), vec![0, 255, 0]);
    m.insert("warning_text".into(), vec![255, 255, 0]);
    // Efektler opaklaştır (kontrast için)
    m.insert("shadow".into(), vec![0, 0, 0, 220]);
    m.insert("glow".into(), vec![255, 255, 0, 80]);
    m.insert("highlight".into(), vec![255, 255, 255, 60]);
    m
}

/// Erişilebilirlik state'i — font scale + high-contrast + palette override.
#[derive(Debug, Clone)]
pub struct Accessibility {
    pub font_scale_idx: usize,        // FONT_SCALE_OPTIONS index (varsayılan 3 -> 1.0)
    pub footer_font_scale_idx: usize, // FOOTER_FONT_SCALE_OPTIONS index (varsayılan 3 -> 1.0, rgsx_settings 1.5 ise 8)
    pub high_contrast: bool,
}

impl Default for Accessibility {
    fn default() -> Self {
        Self { font_scale_idx: 3, footer_font_scale_idx: 3, high_contrast: false }
    }
}

impl Accessibility {
    pub fn new() -> Self { Self::default() }

    pub fn from_env() -> Self {
        let mut a = Self::default();
        if let Ok(v) = std::env::var("RGSX_FONT_SCALE") {
            if let Ok(f) = v.parse::<f32>() { a.set_font_scale_value(f); }
        }
        if let Ok(v) = std::env::var("RGSX_FOOTER_FONT_SCALE") {
            if let Ok(f) = v.parse::<f32>() { a.set_footer_scale_value(f); }
        }
        if let Ok(v) = std::env::var("RGSX_HIGH_CONTRAST") {
            a.high_contrast = v == "1" || v.to_ascii_lowercase() == "true";
        }
        a
    }

    pub fn font_scale(&self) -> f32 {
        FONT_SCALE_OPTIONS.get(self.font_scale_idx).copied().unwrap_or(1.0)
    }
    pub fn footer_font_scale(&self) -> f32 {
        FOOTER_FONT_SCALE_OPTIONS.get(self.footer_font_scale_idx).copied().unwrap_or(1.0)
    }

    pub fn set_font_scale_idx(&mut self, idx: usize) {
        self.font_scale_idx = idx.min(FONT_SCALE_OPTIONS.len() - 1);
    }
    pub fn set_footer_scale_idx(&mut self, idx: usize) {
        self.footer_font_scale_idx = idx.min(FOOTER_FONT_SCALE_OPTIONS.len() - 1);
    }
    pub fn set_font_scale_value(&mut self, v: f32) {
        if let Some((idx, _)) = FONT_SCALE_OPTIONS.iter().enumerate().min_by(|(_, a), (_, b)| ( (*a - v).abs()).partial_cmp(&((*b - v).abs())).unwrap()) {
            self.font_scale_idx = idx;
        }
    }
    pub fn set_footer_scale_value(&mut self, v: f32) {
        if let Some((idx, _)) = FOOTER_FONT_SCALE_OPTIONS.iter().enumerate().min_by(|(_, a), (_, b)| ( (*a - v).abs()).partial_cmp(&((*b - v).abs())).unwrap()) {
            self.footer_font_scale_idx = idx;
        }
    }
    pub fn inc_font_scale(&mut self) { if self.font_scale_idx + 1 < FONT_SCALE_OPTIONS.len() { self.font_scale_idx += 1; } }
    pub fn dec_font_scale(&mut self) { if self.font_scale_idx > 0 { self.font_scale_idx -= 1; } }
    pub fn inc_footer_scale(&mut self) { if self.footer_font_scale_idx + 1 < FOOTER_FONT_SCALE_OPTIONS.len() { self.footer_font_scale_idx += 1; } }
    pub fn dec_footer_scale(&mut self) { if self.footer_font_scale_idx > 0 { self.footer_font_scale_idx -= 1; } }
    pub fn toggle_high_contrast(&mut self) { self.high_contrast = !self.high_contrast; }

    /// Yüksek kontrast aktifse palet override ile tema rengini çözer (anında).
    pub fn effective_color(&self, theme: &Theme, name: &str) -> (u8, u8, u8, u8) {
        if self.high_contrast {
            if let Some(v) = high_contrast_palette().get(name) {
                return match v.len() {
                    4 => (v[0], v[1], v[2], v[3]),
                    3 => (v[0], v[1], v[2], 255),
                    _ => theme.color(name),
                };
            }
        }
        theme.color(name)
    }

    /// Yüksek kontrast aktifse arka plan preset'ini de override eder (siyah).
    pub fn effective_background(&self, theme: &Theme, preset: &str) -> ((u8, u8, u8), (u8, u8, u8)) {
        if self.high_contrast {
            return ((0, 0, 0), (0, 0, 0));
        }
        theme.background(preset)
    }

    /// Ölçeklenmiş boyut (tüm metne font_scale, footer'a footer_scale).
    pub fn scaled(&self, base: u32) -> u32 { ((base as f32) * self.font_scale()) as u32 }
    pub fn scaled_footer(&self, base: u32) -> u32 { ((base as f32) * self.footer_font_scale()) as u32 }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::default_theme;

    #[test]
    fn default_scales_are_one() {
        let a = Accessibility::default();
        assert_eq!(a.font_scale(), 1.0);
        assert_eq!(a.footer_font_scale(), 1.0);
        assert!(!a.high_contrast);
    }

    #[test]
    fn font_scale_options_parity() {
        assert_eq!(FONT_SCALE_OPTIONS.len(), 14);
        assert_eq!(FONT_SCALE_OPTIONS[3], 1.0);
        assert_eq!(FONT_SCALE_OPTIONS[0], 0.7);
        assert_eq!(FONT_SCALE_OPTIONS[13], 2.0);
        assert_eq!(FOOTER_FONT_SCALE_OPTIONS.len(), 19);
        assert_eq!(FOOTER_FONT_SCALE_OPTIONS[3], 1.0);
        assert_eq!(FOOTER_FONT_SCALE_OPTIONS[18], 2.5);
    }

    #[test]
    fn inc_dec_font_scale_clamps() {
        let mut a = Accessibility::default();
        a.dec_font_scale();
        assert_eq!(a.font_scale(), 0.9);
        for _ in 0..20 { a.inc_font_scale(); }
        assert_eq!(a.font_scale(), 2.0);
        for _ in 0..30 { a.dec_font_scale(); }
        assert_eq!(a.font_scale(), 0.7);
    }

    #[test]
    fn inc_dec_footer_scale() {
        let mut a = Accessibility::default();
        a.inc_footer_scale();
        assert_eq!(a.footer_font_scale(), 1.1);
        a.dec_footer_scale();
        assert_eq!(a.footer_font_scale(), 1.0);
        for _ in 0..30 { a.inc_footer_scale(); }
        assert_eq!(a.footer_font_scale(), 2.5);
    }

    #[test]
    fn high_contrast_palette_overrides_instantly() {
        let mut a = Accessibility::default();
        let theme = default_theme();
        let normal_text = a.effective_color(&theme, "text");
        assert_eq!(normal_text, (255, 255, 255, 255)); // zaten beyaz ama high contrast de aynı
        let normal_bg = a.effective_background(&theme, "default");
        assert_ne!(normal_bg, ((0,0,0),(0,0,0)));
        a.toggle_high_contrast();
        assert!(a.high_contrast);
        let hc_bg = a.effective_background(&theme, "default");
        assert_eq!(hc_bg, ((0,0,0),(0,0,0)));
        let hc_border = a.effective_color(&theme, "border_selected");
        assert_eq!(hc_border, (255, 255, 0, 255));
        let hc_button = a.effective_color(&theme, "button_selected");
        assert_eq!(hc_button, (255, 255, 0, 255));
        a.toggle_high_contrast();
        assert_eq!(a.effective_background(&theme, "default"), normal_bg);
    }

    #[test]
    fn scaled_sizes_apply_font_scale() {
        let mut a = Accessibility::default();
        a.set_font_scale_idx(13); // 2.0
        assert_eq!(a.scaled(100), 200);
        a.set_font_scale_idx(0); // 0.7
        assert_eq!(a.scaled(100), 70);
        a.set_footer_scale_idx(18); // 2.5
        assert_eq!(a.scaled_footer(100), 250);
    }

    #[test]
    fn set_by_value_snaps_nearest() {
        let mut a = Accessibility::default();
        a.set_font_scale_value(1.15);
        // 1.1 ve 1.2'ye eşit mesafe, biri seçilir (deterministik)
        assert!(a.font_scale() == 1.1 || a.font_scale() == 1.2);
        a.set_footer_scale_value(2.5);
        assert_eq!(a.footer_font_scale(), 2.5);
    }

    #[test]
    fn high_contrast_palette_keys_complete() {
        let p = high_contrast_palette();
        for k in ["text","border","border_selected","button_selected","background_top","neon","fond_lignes"] {
            assert!(p.contains_key(k), "high contrast palette eksik: {k}");
        }
    }
}
