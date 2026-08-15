# TASK-002-gap-24 — WebUI Region-priority modal (One ROM Per Game)

- **id:** TASK-002-gap-24
- **title:** WebUI Region-priority modal (One ROM Per Game)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, filters, region
- **parent:** TASK-002

## Karar (2026-08-15)

Region-priority modal, `App.vue` → `webui/src/components/Platforms.vue` component'i içine eklenir
(onaylanan tasarım kararı). Renk otoritesi Python hex'leri (bkz. gap-18 Karar).

## Python Kaynağı (dosya:satır)

- `ports/RGSX/rgsx_web/handlers_ui.py:337-344` — `#region-priority-modal` markup + yapılandırıcı
- `ports/RGSX/static/js/app.js:739-1271` — region filter mantığı (include/exclude, priority)

## Rust Mevcut Durum (⚠️) — `webui/src/App.vue`

- `:28` `REGION_PRIORITY = ['USA','Canada','World','Europe','Japan','Other']` **hardcoded**.
- `:223-234` region filtreleri (cycleRegion/resetFilters/saveFilters) var ama **priority configure
  modalı YOK** — kullanıcı öncelik sırasını değiştiremez.
- `/api/save_filters` backend zaten var (`manager-http/src/lib.rs:60`).

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/Platforms.vue` (yeni) — region-priority modal (sürükle/sırala)
- `webui/src/App.vue` — Platforms sekmesi entegrasyonu (component split sonrası)
- `webui/src/api.js` — `saveFilters` `region_priority` dizisini gönderir (zaten gönderiyor, UI eksik)

## Bağımlılık

- `App.vue` → component split (onaylanan tasarım kararı; Platforms.vue).
- `/api/save_filters` backend mevcut (`lib.rs:60`) — backend bağımlılığı yok.

## Doğrulama

- Kullanıcı region önceliğini sıralayabilir; sıralama `/api/save_filters` ile persist edilir.
- One ROM Per Game seçimi yeni önceliğe göre çalışır (Python ile eşdeğer).
