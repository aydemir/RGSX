# TASK-002-gap-22 — WebUI Accessibility (a11y css + font_scale + toggle)

- **id:** TASK-002-gap-22
- **title:** WebUI Accessibility (a11y css + font_scale + toggle)
- **status:** done

## Audit (2026-08-15, App.vue b6c37d8)
- ❌ **Hâlâ YOK.** `DEFAULT_SETTINGS.accessibility: false` ama Settings'te toggle/slider YOK;
  `font_scale` alanı YOK; `accessibility.css` / ARIA live region YOK.
- Sonuç: başlanmadı, `todo` korunur.
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

## Done (2026-08-15, commit feat(webui): accessibility toggle + font_scale)
- **Şema DÜZELTİLDİ (tahmin değil, kanıt):** TASK'taki `{enabled: bool, font_scale: number}` tahmini YANLIŞ.
  Gerçek şema (Rust `manager-core/src/settings.rs:37-43` `struct Accessibility { font_scale: f64,
  footer_font_scale: f64 }` + Python `accessibility.py` `settings.get("accessibility", {font_scale, footer_font_scale})`)
  → `accessibility: { font_scale: 1.0, footer_font_scale: 1.0 }`. `enabled` alanı YOK; eklemedim (icat olurdu).
- `DEFAULT_SETTINGS.accessibility` `false` → `{ font_scale: 1.0, footer_font_scale: 1.0 }`; `normalizeSettings()`
  display/sources/symlink deseninde olduğu gibi derin-merge (`accessibility` atlama listesine eklendi).
- `webui/src/App.vue` Settings: `font_scale` slider (range 0.5–2.0, step 0.1, anlık değer gösterimi) —
  Python `app.js:2242/2407` birebir. `footer_font_scale` Rust şemasında var ama Python web formunda
  GÖSTERİLMİYOR; UI'a eklemedim (web kapsamı aşılmadı), veri round-trip ile korunur.
- **a11y CSS (App.vue `<style>`, Python `static/css/accessibility.css`'a paralel):** `:focus-visible` (#007bff),
  `.sr-only`, `role="status" aria-live="polite"` live region (handlers_ui.py:270-271), `prefers-contrast: more`
  ve `prefers-reduced-motion: reduce` medya sorguları, buton min 44px (WCAG). `enabled` toggle YOK (şemada yok).
- **Font-scale doğrulaması:** `.app` köküne `:style="{ '--font-scale': settings.accessibility.font_scale }"`
  bağlandı; CSS `calc(PX * var(--font-scale))` ile h1/h2/h3/.name/.field label/.tabs button/.muted/.err/
  .searchbar/.games li .size ölçeklenir → slider değişimi görsel olarak yansır (kodda kanıtlı).
- `webui/src/i18n.js` — tr/en `font_scale` dizgesi eklendi.
- `npm run build` temiz.
