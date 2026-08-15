# TASK-002-gap-21 — WebUI Browse-directories UI

- **id:** TASK-002-gap-21
- **title:** WebUI Browse-directories UI (Roms klasörü seçici)
- **status:** todo

## Audit (2026-08-15, App.vue b6c37d8)
- ❌ **Hâlâ YOK.** Yeni App.vue'da `/api/browse-directories` çağrısı / klasör seçici modalı yok.
- Backend mevcut: `manager-http/src/lib.rs:49`. UI eksik.
- Sonuç: başlanmadı, `todo` korunur.
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, settings, browse
- **parent:** TASK-002

## Karar (2026-08-15)

`App.vue` → `webui/src/components/BrowseDirs.vue` component'ine bölünür (onaylanan tasarım kararı);
Settings sekmesi içinde "Roms klasörü seç" modalı olarak entegre edilir. Renk otoritesi Python hex'leri
(bkz. gap-18 Karar).

## Python Kaynağı (dosya:satır)

- `ports/RGSX/static/js/app.js:2611` — `browseRomsFolder()`
- `ports/RGSX/static/js/app.js:2622` — `fetch('/api/browse-directories?path=…')`
- `ports/RGSX/rgsx_web/handlers_ui.py` — browse modal markup'ı

## Rust Mevcut Durum (❌)

- Backend **var**: `manager-http/src/lib.rs:49` `/api/browse-directories` mevcut (`api::browse_directories`).
- UI **yok**: `webui/src/App.vue` browse UI içermiyor.

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/BrowseDirs.vue` (yeni) — klasör ağacı modalı (parent/drive/seç)
- `webui/src/App.vue` — Settings sekmesi entegrasyonu (component split sonrası)
- `webui/src/i18n.js` — ilgili dizgeler (tr/en)

## Bağımlılık

- `App.vue` → component split (onaylanan tasarım kararı).
- `/api/browse-directories` backend zaten var (`lib.rs:49`).

## Doğrulama

- Ayarlar'da klasör seçici modal açılır; path `/api/browse-directories` ile gezilebilir (parent/drive/seç).
- Seçim Settings'e yazılır.
