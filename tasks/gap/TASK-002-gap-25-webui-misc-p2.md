# TASK-002-gap-25 — WebUI Misc P2 (update-cache + i18n + renk/SSE kontrat)

- **id:** TASK-002-gap-25
- **title:** WebUI Misc P2 (update-cache butonu, i18n tam entegrasyon, renk paleti hizası, SSE kontrat)
- **status:** partial

## Audit (2026-08-15, App.vue b6c37d8)
- ⚠️ **KISMEN.** SSE kontratı zaten ✅ (önceki tur, bu alt-madde TAMAM).
- App.vue b6c37d8'de:
  - (a) Update Cache butonu / `/api/update-cache` çağrısı **YOK** (backend `lib.rs:54` var).
  - (b) i18n: `tt()` + `STRINGS` fallback + `/api/translations` (`dataLang`) var, ama Python
    `data-translate` sunucu bind'i **YOK**.
  - (c) Renkler hâlâ Python hex otoritesinden sapıyor (`.st-downloaded #66ff66`, `.dlall #2f8f46`,
    `.danger #6e2b2b`, `status #58a6ff`, vb. — `#28a745/#dc3545/#ffc107/#17a2b8/#007bff` değil).
- Sonuç: update-cache, i18n server-bind, renk hizası hâlâ eksik.
- **priority:** P2
- **created:** 2026-08-15
- **environment:** both
- **tags:** webui, i18n, color, sse, misc
- **parent:** TASK-002

## Karar (2026-08-15)

Renk otoritesi Python hex'leri: `#28a745` (ok), `#dc3545` (err), `#ffc107` (warn/downloading),
`#17a2b8` (info), `#007bff` (run). Rust sapmaları (`#2f8f46`, `#d29922`, `#58a6ff`) BUNLARA hizalanır.

### SSE event-kontrat DOĞRULANDI (2026-08-15) ✅ — bu alt-madde TAMAM, kod yazmaya gerek yok

- Vue dinleyicileri: `{snapshot, progress, queue, history, downloaded}`
  (`webui/src/api.js:8-16`, handler isimleri `webui/src/App.vue:79-100`).
- Rust yayınları: `snapshot` `manager-http/src/sse.rs:62`; `progress` `api.rs:542/1112/1193`;
  `queue` `api.rs:589/677/706/1107/1157`; `history` `api.rs:1108`; `downloaded` `api.rs:1110`.
- Wire format `manager-core/src/contract.rs:38-39` → `event: <type>\ndata: <json>\n\n`
  (EventSource ile birebir). Ek olaylar `scan` (`api.rs:351`) ve `gamepad` (`native_input.rs:131`)
  — Vue dinlemiyor, zararsız.
- **Sonuç: 5 dinleyici = 5 yayın, isimler birebir aynı → EŞLEŞİYOR.**

## Python Kaynağı (dosya:satır)

- `ports/RGSX/static/js/app.js:459` — Update Cache butonu (`/api/update-cache`)
- `ports/RGSX/static/js/app.js` — i18n `t()` + `data-translate` attribute'leri + `/api/translations` sunucu bind

## Rust Mevcut Durum (❌ / ⚠️)

- Update Cache: backend var (`manager-http/src/lib.rs:54` `/api/update_cache`) ama UI butonu **yok**.
- i18n: `webui/src/i18n.js` `STRINGS` tr/en var, ama `App.vue` `tt()` fallback kullanır; sunucu
  `/api/translations` veri dili bind'i **yok** (`data-translate` karşılığı yok).
- Renk: `webui/src/App.vue` `<style>` sapmalar içeriyor (`#2f8f46`, `#d29922`, `#58a6ff`).

## Kapsam / Dosyalar (değişecek)

- `webui/src/components/Settings.vue` — Update Cache butonu (gap-23 component'i içinde)
- `webui/src/i18n.js` — sunucu `/api/translations` entegrasyonu (App.vue `loadSettings` zaten çeker;
  template'e `data-translate` benzeri bind eklenir)
- Tüm component'ler (`Platforms/Downloads/Queue/History/Settings/QBittorrent/Support/BrowseDirs/
  Accessibility.vue`) — renk sabitleri Python hex'ine hizalanır (gap-18 Karar)

## Bağımlılık

- `App.vue` → component split (onaylanan tasarım kararı).
- `TASK-002-gap-22` (a11y css renkleri), `TASK-002-gap-23` (Settings.vue) — renk/i18n bu component'lerde.
- `/api/update_cache` backend zaten var (`lib.rs:54`).
- SSE kontratı doğrulandı → bu alt-madde bağımsız ve TAMAM.

## Doğrulama

- Update Cache butonu `/api/update-cache` çağırır, UI yenilenir.
- UI dizgeleri sunucu veri diline bağlanır (Python `data-translate` eşdeğeri).
- Tüm renklar Python hex otoritesiyle birebir.
