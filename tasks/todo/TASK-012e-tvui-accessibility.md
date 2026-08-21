# TASK-012e — TVUI Accessibility (Font Scale / Yüksek Kontrast)

- **id:** TASK-012e
- **title:** TVUI erişilebilirlik — font_scale / footer_font_scale / yüksek kontrast
- **status:** superseded
- **superseded_by:** Yön (B) native SDL2 + `.json` tema — bkz. TASK-012g..l (2026-08-21)
- **priority:** P2
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, accessibility, parity, faz12b

## Kaynak

- `plan.md` §5.5, `ports/RGSX/tvui.py` (`config.font_scale_options`,
  `config.footer_font_scale_options`, `config.accessibility_settings`), `display/fonts.py`

## Açıklama

Eski `font_scale` / `footer_font_scale` + yüksek kontrast ayarlarının **görünür etkisi**
parity'de kalır; SPA'da CSS değişkeni / class olarak uygulanır (impl serbest).

**Behavior contract (parity):**
- `font_scale` değişimi tüm ekran metnine anında yansır.
- `footer_font_scale` alt bar metnine ayrıca uygulanır.
- Yüksek kontrast modu renk kontrastını eski görünümle eşler.

## Kapsam / Dosyalar

- `webui/` — font-scale CSS değişkeni + yüksek kontrast class (`?mode=tv`).
- `manager-core/src/settings.rs` — `RGSX_NATIVE_SETTINGS=1` ile alan (gap-17 ile çakışma kontrolü).

## Doğrulama

- Ayar değişimi canlı yansır (refresh gerektirmez).
- Erişilebilirlik davranışı eski TVUI ile görsel olarak eşleşir.

---

## İlerleme

- 2026-08-21 — plan.md §5.5'den çıkarıldı.
