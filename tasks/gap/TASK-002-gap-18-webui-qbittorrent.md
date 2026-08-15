# TASK-002-gap-18 — WebUI qBittorrent bölümü (parola durum/regenerate/change/start)

- **id:** TASK-002-gap-18
- **title:** WebUI qBittorrent bölümü (parola durum / regenerate / change / start)
- **status:** todo
- **priority:** P0
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, qbittorrent, settings
- **parent:** TASK-002

## Karar (2026-08-15)

`App.vue` → `webui/src/components/QBittorrent.vue` component'ine bölünür (onaylanan tasarım kararı).
Renk otoritesi Python hex'leri: `#28a745` (ok), `#dc3545` (err), `#ffc107` (warn/downloading),
`#17a2b8` (info), `#007bff` (run). Rust sapmaları (`#2f8f46`, `#d29922`, `#58a6ff`) hizalanır.

## Python Kaynağı (dosya:satır)

- `ports/RGSX/static/js/app.js:2359` — "🧲 qBittorrent WebUI" ayar sekmesi bölümü
- `ports/RGSX/static/js/app.js:326` — `fetch('/api/qbittorrent/start')`
- `ports/RGSX/static/js/app.js:380` — `/api/qbittorrent/regenerate-password`
- `ports/RGSX/static/js/app.js:424` — `/api/qbittorrent/change-password`
- `ports/RGSX/rgsx_web/handlers_settings.py` — qBittorrent route'ları (password-status/start/regenerate/change)

## Rust Mevcut Durum (❌)

- Backend **var**: `manager-http/src/lib.rs:68-71` `/api/qbittorrent/*` endpoint'leri mevcut.
- UI **yok**: `webui/src/App.vue` settings sekmesi (`:574-638`) qBittorrent bölümünü içermiyor.

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/QBittorrent.vue` (yeni) — parola durumu gösterimi + regenerate/change/start formu
- `webui/src/App.vue` — Settings sekmesine QBittorrent bölümü entegrasyonu (component split sonrası)
- `webui/src/i18n.js` — ilgili dizgeler (tr/en)

## Bağımlılık

- `App.vue` → component split (onaylanan tasarım kararı, bu TASK'tan önce uygulanır).
- Backend bağımlılığı yok (`/api/qbittorrent/*` zaten mevcut, `lib.rs:68-71`).

## Doğrulama

- Settings sekmesinde qBittorrent bölümü görünür; parola durumu `/api/qbittorrent/password-status` ile çekilir.
- regenerate / change-password / start çağrıları Python'dakiyle aynı endpoint'lere düşer.
- Renkler Python hex otoritesiyle uyumlu.
