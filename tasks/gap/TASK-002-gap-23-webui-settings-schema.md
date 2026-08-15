# TASK-002-gap-23 — WebUI Settings şema uyumu (sources.mode, symlink, display.grid, accessibility)

- **id:** TASK-002-gap-23
- **title:** WebUI Settings şema uyumu (sources.mode, symlink, display.grid, accessibility)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, settings, schema, parity
- **parent:** TASK-002

## Karar (2026-08-15)

`App.vue` → `webui/src/components/Settings.vue` component'ine bölünür (onaylanan tasarım kararı).
Renk otoritesi Python hex'leri (bkz. gap-18 Karar). Bu TASK, `TASK-002-gap-17` (backend settings
schema parity) ile bağlıdır: backend alanları persist edilmeli.

## Python Kaynağı (dosya:satır)

- `ports/RGSX/rgsx_settings.py` — `display.grid` boolean, `sources.mode` = `'archive' | 'custom'`,
  `symlink`, `accessibility` alanları
- `ports/RGSX/static/js/app.js` — settings render (sources.mode archive/custom, symlink, font_scale)

## Rust Mevcut Durum (❌ / ⚠️) — `webui/src/App.vue`

- `:49` `DEFAULT_SETTINGS.sources: { mode: 'archive' }` **ama** template `:623-625` select opsiyonları
  `'rgsx' | 'custom'` → kendi içinde ÇELİŞKİLİ + Python'dan sapıyor (Python: `'archive' | 'custom'`).
- `:45` `display.grid: true` (boolean) **ama** template `:595` select string `"2x4"…"5x3"` → çelişkili.
- `:51` `symlink: false` (bool) **ama** template `:633` `v-model="settings.symlink.enabled"` →
  undefined /bind hatası kaynağı.
- `:51` `accessibility: false` (bool) ama UI yok (bkz. gap-22).

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/Settings.vue` (yeni, App.vue'den çıkarılır)
- `webui/src/App.vue:42-52` `DEFAULT_SETTINGS` düzeltmesi:
  - `sources.mode` → `'archive' | 'custom'` (select opsiyonlarıyla uyumlu)
  - `display.grid` → `'2x4'` string default (select ile uyumlu)
  - `symlink` → `{ enabled: false }` (template `symlink.enabled` ile uyumlu)
  - `accessibility` → `{ font_scale: 1.0, enabled: false }`
- `webui/src/i18n.js` — ilgili dizgeler

## Bağımlılık

- `TASK-002-gap-17` (backend settings schema parity — alanlar persist edilmeli).
- `App.vue` → component split (onaylanan tasarım kararı).

## Doğrulama

- `/api/settings` round-trip'de veri kaybı yok (gap-17 ile ortak kriter).
- UI seçenekleri Python ile aynı (sources.mode archive/custom, grid, symlink, accessibility).
- `symlink.enabled` / `accessibility` doğru bind edilir, konsol hatası yok.
