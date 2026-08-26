# TASK-014 — Statik modda `/api/*` bilinmeyen yolların HTML 200 yerine JSON 404 dönmesi

- **id:** TASK-014
- **title:** Router fallback birliği — statik (WebUI) modda `/api/*` istekleri SPA fallback'ine düşmesin
- **status:** done
- **priority:** P2
- **created:** 2026-08-26
- **environment:** both
- **tags:** http-router, api-contract, spa-fallback, 404
- **relation:** TASK-013 canlı smoke gözlemi (emekli `/api/qbittorrent/*` çağrısı HTML 200 döndü)

## Kaynak

TASK-013 canlı smoke (2026-08-26): sökülen `/api/qbittorrent/password-status` (GET) ve
`/api/qbittorrent/start` (POST) uçlarına yapılan çağrılar `text/html` + **200** döndürdü.
Sessiz hata kaynağı: eski/bayat istemci (script, TVUI, tarayıcı eklentisi) emekli veya
yanlış bir API yoluna gittiğinde JSON beklerken HTML alır ve parse aşamasında nedeni
belirsiz şekilde boğulur; 404 sinyali hiç ulaşmaz.

## Problem

İki fallback paralel yaşıyor ve üretim yolu yanlış olanı seçiyor:

- `manager-http/src/api.rs:1746` — `api::fallback`: **doğru davranış zaten burada**
  (`/api/*` → JSON `{"error":"Route non trouvée","path":...}` 404; SPA yolu → `index`;
  diğerleri → düz 404).
- `manager-http/src/lib.rs:93` — `static_root` varsa (üretim) `.fallback(api::index)`
  bağlanıyor; `api::fallback` devre dışı kalıyor. `static_root` yoksa (test/placeholder)
  doğru fallback kullanılıyor → contract testleri bu hatayı göremiyor.

## Kapsam

- [ ] `lib.rs`: her iki dalda da fallback `api::fallback` olur (statik dal `route("/",
      get(index))` + `nest("/static", ...)` korunur; yalnız `.fallback` değişir).
- [ ] `is_spa_path` kapsamı Vue router rota listesiyle karşılaştırılır — mevcut catch-all
      davranışta derin bağlantılarla açılabilen her SPA rotası (`/settings`, `/downloads`,
      `/history`, `/platform/...`, ...) 404'e düşmemeli; eksikse listeye eklenir.
- [ ] Contract testleri (static-root'lu app ile): `GET /api/nope` → 404 JSON + `path`
      alanı; `POST /api/nope` → 404 JSON; `GET /settings` → 200 text/html.
      Baseline 105 → ~108.

## KALACAKLAR (dokunulmaz)

- CORS başlıkları (`cors_response`) ve Python sözleşme gövdesi `contract::error`
  ("Route non trouvée") birebir korunur.
- `/static/*` ServeDir davranışı ve `/` kök handler'ı değişmez.
- `static_root`'suz (placeholder) moddaki mevcut davranış değişmez.

## Doğrulama

- `cargo test -p manager-http` yeşil (yeni fallback testleri dahil).
- Canlı: manager-bin boot → `GET /api/qbittorrent/password-status` artık **404 application/json**;
  `GET /settings` hâlâ hydrate edilmiş index.html; `GET /static/<asset>` çalışıyor.

## İlerleme

- 2026-08-26 — Görev açıldı (TASK-013 smoke bulgusu); KANBAN'a sıra 12 olarak eklendi.
- 2026-08-26 — Tamamlandı: `lib.rs` statik dal `.fallback(api::fallback)` bağlandı
  (`route("/", get(index))` + `nest("/static", ...)` korundu). `is_spa_path` denetimi:
  WebUI'de vue-router yok (tek sayfa, tab state `App.vue`) — mevcut liste Python
  parity yollarını zaten karşılıyor, ekleme gerekmedi. Contract 105 → 109: statik modda
  GET/POST `/api/nope` JSON 404 + `path`, `/settings` text/html, bilinmeyen non-API düz
  404 (davranış değişikliği kilitlendi). Canlı smoke yeşil: `/api/qbittorrent/password-status`
  404 application/json; POST `/api/qbittorrent/start` 404 JSON; `/settings` 200 text/html;
  `/static/assets/*.js` 200; `/bogus/whatever` 404. Dosya `done/`a taşındı.
