# TASK-002b — HTTP köprüsü: axum `/api/*` + SSE (characterization)

- **id:** TASK-002b
- **title:** manager-http: axum `/api/*` + SSE sözleşmesi
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **tags:** rust, axum, sse, contract, manager-http
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10 (Rust kısmi refaktör)
- **Parent:** TASK-002 (Faz 10 üst görev; alt-görev sırası: state machine → HTTP → bridge → windows)
- **Karakterizasyon (Faz 7):** `tests/test_api_contract.py` — `RGSXHandler`/`ManagerHandler` sözleşmesi

## Açıklama

Mevcut Python WebUI/Manager HTTP API'sini (`ports/RGSX/rgsx_web/handlers.py` + mixin'ler;
`ports/RGSX/rgsx_manager.py:ManagerHandler`) axum ile birebir çoğaltmak. Sözleşme:
`/api/*` GET/POST + `/api/events` (SSE). Yanıt şekilleri (status/header/payload) Python
birebir; Faz 7 karakterizasyon testleri bunu kullanılabilir altın referans olarak kilitler.

Bu adım **SADECE HTTP sözleşmesi** (router, dispatch, resp shapers, SSE). İş mantığı
çekirdekleri (queue/download/settings) ilk dilimde placeholder + manager-core state
machine'e bağlı yürür; gerçek eylemler TASK-002c bridge/bin entegrasyonunda.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/lib.rs` — axum `Router`: GET/POST dispatch + `/api/events` SSE
- `manager-rs/manager-http/src/api.rs` (önerilen) — `/api/*` route handler'ları + response shapers
- `manager-rs/manager-http/src/sse.rs` (önerilen) — SSE event formati (`data:`/`event:` kanalları)
- `manager-rs/manager-http/src/state.rs` → manager-core `ManagerState`/`DownloadState` köprüsü
- `manager-rs/manager-http/tests/contract.rs` — characterization: test_api_contract.py senaryoları
  → axum handler testleri (girdi→status/header/payload 1:1)
- `manager-rs/manager-bin/src/main.rs` — boş state'le sunucu ayağa kaldırma (smoke)

## Doğrulama

- `cargo test -p manager-http` — contract testleri yeşil (test_api_contract.py kapsamı)
- `cargo check --workspace` geçer
- Canlı smoke (Windows): `manager-bin` ayağa kalkar; `Invoke-RestMethod` ile birkaç `/api/*`
  endpoint + SSE açılışı
- Python suite baseline değişmez (761 passed / 11 pre-existing)
- Characterization (Python↔Rust 1:1 tablo) bu adımda contract testleriyle kilitlenir

---

## İlerleme

- 2026-08-12 — Alt-görev tanımlandı (TASK-002 planı onay sonrası)
- 2026-08-12 — **Tamamlandı.** Contract extraction: `tests/test_api_contract.py` (607 satır) + `rgsx_manager.py` SSE formatı (`_sse_event`, `_build_snapshot`, `_handle_sse`) + `history.py:_strip_history_error_noise` birebir okundu.
  - `manager-core/src/contract.rs`: `ok()`/`error()` zarf + `sse_event()` format + `snapshot()` + `strip_history_error_noise()` (history.py:17 1:1)
  - `manager-http/src/state.rs`: `StateData` (config eşleniği: history/queue/progress/downloaded/active) + `AppState` (RwLock + SSE broadcast kanalı)
  - `manager-http/src/sse.rs`: `publish()` (kanala raw SSE metni), `snapshot_json()`, `/api/events` handler (bağlantıda snapshot → canlı olaylar; brüt `text/event-stream`)
  - `manager-http/src/api.rs`: 30+ route handler — status/header/payload Python 1:1, CORS `*`, 404 `Route non trouvée` + path
  - `manager-http/src/lib.rs`: axum Router dispatch; `:platform` path syntax (matchit 0.7.3 `{param}`'ı desteklemiyor — canlı testle doğrulandı)
  - `manager-http/tests/contract.rs`: 52 characterization testi (Python senaryoları 1:1) — **52 passed**
  - `manager-bin/src/main.rs`: boş state smoke (port 5010; `RGSX_MANAGER_BIN_PORT` env)
  - Doğrulama: `cargo test --workspace` 82 test yeşil (30 core + 52 contract); `cargo check --workspace` temiz; canlı smoke: health/queue/platforms/history/translations/game-status + POST download (200/400) + SSE snapshot akışı + CORS hepsi OK
  - Python baseline değişmedi: 761 passed / 11 pre-existing failure (Linux/tray)
  - **Not:** axum 0.7.9 + matchit 0.7.3'te `{param}` yerine `:param` kullanılır