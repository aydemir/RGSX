# TASK-002-gap-19 — WebUI Support / redaction ZIP UI

- **id:** TASK-002-gap-19
- **title:** WebUI Support / redaction ZIP UI (🆘 butonu)
- **status:** todo

## Audit (2026-08-15, App.vue b6c37d8)
- ❌ **Hâlâ YOK.** Yeni App.vue'da 🆘 butonu / `/api/support` çağrısı yok (header ve tabs'da yok).
- Backend mevcut: `manager-http/src/lib.rs:63` `/api/support`. UI eksik.
- Sonuç: başlanmadı, `todo` korunur.
- **priority:** P0
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, support, redaction
- **parent:** TASK-002

## Karar (2026-08-15)

`App.vue` → `webui/src/components/Support.vue` component'ine bölünür (onaylanan tasarım kararı).
Bu TASK, `TASK-002-gap-13` (P0 backend redaction) ile bağlıdır: UI, backend redaction'ı tetikler.
Renk otoritesi Python hex'leri (bkz. gap-18 Karar).

## Python Kaynağı (dosya:satır)

- `ports/RGSX/rgsx_web/handlers_ui.py:288` — `generateSupportZip()` butonu (🆘, header)
- `ports/RGSX/rgsx_web/handlers_ui.py:300` — Support tab'ı (`onclick="generateSupportZip()"`)
- `ports/RGSX/rgsx_web/handlers_settings.py:331` — `_api_support()` (zip üretimi + `_redact_settings_file_text`)
- `ports/RGSX/static/js/app.js:2553` — `fetch('/api/support', …)`

## Rust Mevcut Durum (❌)

- Backend **var**: `manager-http/src/lib.rs:63` `/api/support` POST mevcut (`api::support`).
- UI **yok**: `webui/src/App.vue` destek butonu/tab'ı içermiyor (`generateSupportZip` karşılığı yok).

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/Support.vue` (yeni) — 🆘 butonu + indirme akışı
- `webui/src/App.vue` — header/sekme entegrasyonu (component split sonrası)
- `webui/src/i18n.js` — ilgili dizgeler (tr/en)

## Bağımlılık

- `TASK-002-gap-13` (backend redaction P0) — redaction backend'i stabil olmalı.
- `App.vue` → component split (onaylanan tasarım kararı).

## Doğrulama

- 🆘 butonu → `/api/support` çağrılır, redacted zip indirilir.
- Hassas değerler (parola/API key/token) maskelenir — `gap-13` ile aynı kabul kriteri.
