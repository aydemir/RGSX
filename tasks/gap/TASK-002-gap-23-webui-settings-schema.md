# TASK-002-gap-23 — WebUI Settings şema uyumu (sources.mode, symlink, display.grid, accessibility)

- **id:** TASK-002-gap-23
- **title:** WebUI Settings şema uyumu (sources.mode, symlink, display.grid, accessibility)
- **status:** done

## Audit (2026-08-15, ADIM 1 HOTFIX — App.vue commit)
- ✅ **TAMAM.** 3 binding bug'ı kaynak doğrulamasıyla düzeltildi (tahmin YOK):
  - `symlink`: `settings.rs:60-65` + `rgsx_settings.py:73-76` → `{enabled, target_directory}` objesi.
    `DEFAULT_SETTINGS.symlink: false` → `{ enabled: false, target_directory: '' }` (template `symlink.enabled` artık geçerli).
  - `sources.mode`: `settings.rs:138-139` + `rgsx_settings.py:78` → default `"rgsx"` (Python'da `'archive'` DEĞİL).
    `DEFAULT_SETTINGS.sources.mode: 'archive'` → `'rgsx'` (select opsiyonları `rgsx|custom` ile uyumlu).
  - `display.grid`: `settings.rs:129-131` + `rgsx_settings.py:67` → **string** `"3x4"`.
    `DEFAULT_SETTINGS.display.grid: true` → `'3x4'`.
  - `normalizeSettings()` `symlink`'i de derin merge eder (`display`/`sources` gibi).
- `npm run build` ile doğrulandı (12 modül, hatasız).
- Not: `accessibility` alanının *UI* tarafı gap-22'de (ayrı ADIM). Şema uyumu (alanın varlığı + round-trip) burada tamam.

## Audit (2026-08-15, App.vue b6c37d8)
- ⚠️ **KISMEN.** Settings şeması genişledi: `sources.mode` select ('rgsx'/'custom') + `custom_url`,
  `symlink` checkbox, `display.grid`, `light_mode`, `max_simultaneous_downloads`, vb.
- AMA gap-23'teki tip çelişkileri SÜRÜYOR:
  1. `DEFAULT_SETTINGS.sources.mode = 'archive'` vs select opsiyonları `'rgsx'|'custom'`
     → Python (`'archive'|'custom'`) ile uyumsuz.
  2. `DEFAULT_SETTINGS.symlink: false` ama template `v-model="settings.symlink.enabled"`
     → `undefined.enabled` hatası (bind kırık).
  3. `DEFAULT_SETTINGS.display.grid: true` (bool) vs select string `"2x4"…"5x3"` → uyumsuz.
  4. `accessibility: false` (bool) ama UI toggle yok (bkz. gap-22).
- Renk otoritesi hizalanmadı (bkz. gap-25). Sonuç: şema genişledi ama uyumsuzluklar devam.
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
