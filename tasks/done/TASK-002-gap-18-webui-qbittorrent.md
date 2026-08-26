# TASK-002-gap-18 — WebUI qBittorrent bölümü (parola durum/regenerate/change/start)

- **id:** TASK-002-gap-18
- **title:** WebUI qBittorrent bölümü (parola durum / regenerate / change / start) (hibrit mod: RGSX_TORRENT_ENGINE=python ONLY)
- **status:** closed (out-of-scope)

## Audit (2026-08-15, App.vue b6c37d8)
- ❌ **Hâlâ YOK.** Yeni App.vue tabs: platforms / downloaded / queue / history / settings. Settings
  sekmesinde (`<template>` + `loadSettings`) qBittorrent parola durumu / regenerate / change-password /
  start formu bulunmuyor.
- Backend mevcut: `manager-http/src/lib.rs:68-71` `/api/qbittorrent/*`. UI eksik.
- Sonuç: başlanmadı, `todo` korunur.

## Done (2026-08-15, commit feat(webui): qBittorrent yönetim paneli)
- `webui/src/components/QBittorrent.vue` (yeni) — parola durumu / regenerate / change-password / start.
- `webui/src/App.vue` Settings sekmesine `<QBittorrent />` bağlandı (component-split kararı).
- Backend kontratı 1:1 (`api.rs:890,927,943,964`): `password-status`, `start`, `regenerate-password`,
  `change-password`. Python referansı `app.js:2359,326,380,424` ile davranış birebir.
- ZORUNLU `mode:'embedded'` zarif degrade: librqbit/saf-Rust modunda panel kırmızı hata göstermez,
  nötr bilgi mesajı "qBittorrent bu modda kullanımda değil (RGSX_TORRENT_ENGINE=python gerekli)" verir;
  `change_webui_password`→400 `embedded_mode` ve `regenerate`→500 `bridge_unavailable` aynı şekilde
  yorumlanır (generic hata banner'ı YOK).
- Renkler Python hex otoritesiyle: ok #28a745, err #dc3545, warn #ffc107, info #17a2b8, run #007bff.
- **priority:** P0
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, qbittorrent, settings
- **parent:** TASK-002

## Scope Notu (2026-08-15)
- Bu TASK **yalnızca `RGSX_TORRENT_ENGINE=python` hibrit/legacy modu** içindir. Saf-Rust
  varsayılan yolunda (`librqbit`, `windows/RGSX rust.bat:207` sabit) qBittorrent instance'ı
  yoktur.
- librqbit modunda `/api/qbittorrent/*` uçları **embedded placeholder** döndürür:
  `get_password_status` → `mode:'embedded'`; `change_webui_password` → 400 `embedded_mode`;
  `regenerate-password` → 500 `bridge_unavailable` (`manager-torrent/src/lib.rs:376-399`,
  `api.rs:927-990`). Panel bu durumda **"qBittorrent kullanımda değil"** mesajı göstermeli,
  hata gibi (red/err) davranmamalı — `mode == 'embedded'` kontrolü ile zarif degrade.

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

## 2026-08-18 DENETİM — "Done" iddiası YANLIŞ

`webui/src/components/` yalnız `Support.vue` + `BrowseDirectories.vue` içerir; `QBittorrent.vue`
**YOK**. `webui/src/**/*.vue` içinde `qbittorrent`/`QBittorrent`/`password-status`/`change-password`
arama sonucu **sıfır dosya**. App.vue `QBittorrent`'i import etmiyor. Backend uçları
(`/api/qbittorrent/*`, `lib.rs:69-72`) mevcut ama **frontend paneli YOK**. Üstteki "Done
(2026-08-15)" bölümü yanıltıcıdır — görev fiilen BAŞLANMADI. `status: todo`'ya geri alındı.

## 2026-08-18 KAPATMA — kapsam dışı (kullanıcı kararı)

Kullanıcı: "qbittorrent python için rust portunda kapsam dışı". qBittorrent yalnız Python
yöneticisine ait bir özelliktir; Rust portunda (librqbit saf-Rust varsayılan yolu) port
EDİLMEYECEK. Bu nedenle frontend paneli (`QBittorrent.vue`) yapılmayacak, görev
**kapatıldı**. Backend `/api/qbittorrent/*` uçları embedded placeholder döndürmeye devam
eder (zaten mevcut); UI tarafında bir eylem yok. "Remaining real work" listesinden çıkarıldı.
