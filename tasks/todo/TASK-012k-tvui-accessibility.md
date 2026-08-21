# TASK-012k — Erişilebilirlik (font_scale / yüksek kontrast)

- **id:** TASK-012k
- **title:** SDL2 erişilebilirlik (font_scale / footer_font_scale / yüksek kontrast)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, sdl2, accessibility, native, faz12b, parity
- **source:** accessibility.py, display/fonts.py, colors.py
- **depends_on:** TASK-012h
- **supersedes:** TASK-012e

## Kaynak

- `accessibility.py` (font_scale, footer_font_scale, yüksek kontrast ayarları)
- `display/fonts.py` (`font_scale_options`, `footer_font_scale_options` ölçekleme)
- `display/colors.py` (yüksek kontrast paleti)

## Açıklama

`accessibility.py` ayarları (font_scale, footer_font_scale, yüksek kontrast) SDL2'de tema/
state olarak portlanır. `fonts.py` ölçekleme `font_scale_options` ile uygulanır.

**Behavior contract (parity):**
- font_scale tüm metne uygulanır; footer ayrı ölçeklenir (footer_font_scale_options).
- Yüksek kontrast: renk paleti eski TVUI ile eşleşir (uygulanınca anında).

## Kapsam / Dosyalar

- `manager-tvui/src/accessibility.rs` (scale state + high-contrast palette override).

## Doğrulama

- Canlı uygulama; eski TVUI erişilebilirlik görünümü ile eşleşir.

---

## İlerleme

- 2026-08-21 — yön (B) kararıyla çıkarıldı (SPA TASK-012e superseded).
