# TASK-002-gap-22 — WebUI Accessibility (a11y css + font_scale + toggle)

- **id:** TASK-002-gap-22
- **title:** WebUI Accessibility (a11y css + font_scale + toggle)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, accessibility, a11y
- **parent:** TASK-002

## Karar (2026-08-15)

`App.vue` → `webui/src/components/Accessibility.vue` component'ine bölünür (onaylanan tasarım kararı).
Renk otoritesi Python hex'leri: `#28a745` (ok), `#dc3545` (err), `#ffc107` (warn/downloading),
`#17a2b8` (info), `#007bff` (run). Rust sapmaları (`#2f8f46`, `#d29922`, `#58a6ff`) BUNLARA hizalanır.

## Python Kaynağı (dosya:satır)

- `ports/RGSX/rgsx_web/handlers_ui.py:264` — `<link … accessibility.css>`
- `ports/RGSX/rgsx_web/handlers_ui.py:270` — live region (screen reader announcements)
- `ports/RGSX/rgsx_web/handlers_ui.py:333` — `accessibility.js` script
- `ports/RGSX/static/js/app.js:2242` — `font_scale` ayarı + accessibility toggle

## Rust Mevcut Durum (❌ / ⚠️)

- `webui/src/App.vue:51` `DEFAULT_SETTINGS` `accessibility: false` ama **UI/toggle YOK**.
- `webui/src/App.vue` `<style>` içinde a11y CSS **yok**; `font_scale` ayarı yok.
- `accessibility` alanı Settings UI'da bağlı değil.

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/Accessibility.vue` (yeni) — font_scale slider + accessibility toggle
- `webui/src/assets/accessibility.css` (yeni) — Python `accessibility.css`'a denk (focus/contrast/aria)
- `webui/src/App.vue` — Settings entegrasyonu (component split sonrası)
- `webui/src/i18n.js` — ilgili dizgeler (tr/en)

## Bağımlılık

- `App.vue` → component split (onaylanan tasarım kararı).
- `TASK-002-gap-17` (backend settings schema — `accessibility` alanı persist edilmeli, gerekirse).

## Doğrulama

- A11y toggle + font_scale ayarlanır; ayarlar `/api/settings` ile round-trip eder.
- Focus/ARIA live region davranışı Python `accessibility.css`/`accessibility.js` ile eşdeğer.
- Renkler Python hex otoritesine hizalı.
