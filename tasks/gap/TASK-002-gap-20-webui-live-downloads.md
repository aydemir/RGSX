# TASK-002-gap-20 — WebUI canlı Downloads sekmesi (SSE progress view)

- **id:** TASK-002-gap-20
- **title:** WebUI canlı Downloads sekmesi (SSE progress view)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, downloads, sse
- **parent:** TASK-002

## Karar (2026-08-15)

`App.vue` → `webui/src/components/Downloads.vue` component'ine bölünür (onaylanan tasarım kararı).
Python'daki 3s `setInterval` poll **yerine** SSE `progress` olayı kullanılır — mekanizma farkı kabul
edilebilir, görsel/davranışsal sonuç aynı. SSE kontratı doğrulandı (bkz. gap-25, renk/SSE alt-maddesi).
Renk otoritesi Python hex'leri (bkz. gap-18 Karar).

## Python Kaynağı (dosya:satır)

- `ports/RGSX/static/js/app.js:533` — `setInterval(() => loadProgress(), 3000)` (downloads sekmesi 3s poll)
- `ports/RGSX/static/js/app.js:557` — `showTab('downloads', false)`
- `ports/RGSX/rgsx_web/handlers_ui.py:305` — `#downloads-content` bölümü

## Rust Mevcut Durum (⚠️)

- `webui/src/App.vue:526-549` queue sekmesi SSE ile progress gösterir, ama **ayrı "downloads" sekmesi YOK**.
- Canlı indirme görünümü queue sekmesi içinde örtük; Python'daki bağımsız Downloads sekmesi eksik.

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/Downloads.vue` (yeni) — aktif indirmeler + canlı `%` / hız
- `webui/src/App.vue` — tabs'a "Downloads" eklenir (component split sonrası)
- `webui/src/api.js` — zaten `progress` handler mevcut; Downloads.vue bu olayı dinler

## Bağımlılık

- `App.vue` → component split (onaylanan tasarım kararı).
- SSE `progress` olayı mevcut (`api.rs:542/1112/1193`) — bağımlılık yok.

## Doğrulama

- Downloads sekmesi aktif indirmeleri canlı `%` ve hız ile gösterir (Python downloads sekmesiyle eşdeğer).
- SSE `progress` olayı ile güncellenir; 3s poll gerekmez.
