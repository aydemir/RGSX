# TASK-002c — Python arka plan köprüsü + manager-bin entegrasyonu

- **id:** TASK-002c
- **title:** qbittorrent_backend subprocess köprüsü + binary entegrasyonu
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **tags:** rust, bridge, subprocess, tokio, manager-bin
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10 "Ara mimari": Rust manager
  binary, `qbittorrent_backend.py`'yi subprocess çağırır (JSON-RPC / local HTTP köprüsü).

## Açıklama

Rust manager binary (manager-bin), mevcut `qbittorrent_backend.py`'yi **subprocess** olarak
başlatır ve local JSON-RPC / HTTP köprüsüyle konuşur. Downloader mantığı Python'da kalırken
Windows tarafı kademeli Rust'a geçer; Linux/Batocera Python yolunu korur.

Ayrıca manager-core state machine + manager-http (TASK-002b) + bridge entegrasyonu tek
binary'de birleştirilir: sağlık döngüsü (watchdog logic), manager durum yayını, `/api/*`
eylemlerinin libsiye bağlanması.

## Kapsam / Dosyalar

- `manager-rs/manager-bridge/src/lib.rs` — subprocess spawn + JSON-RPC/local HTTP protokolü
- `manager-rs/manager-bin/src/main.rs` — tokio runtime: core state + http + bridge birleşimi
- Python tarafı: bridge protokolüne uygun minimal uç (mevkii Python'da kalır)

## Doğrulama

- `cargo test` (workspace) + `cargo check --workspace` geçer
- Köprü: subprocess'teki Python karşıtıyla echo/RPC round-trip testi
- Canlı: `manager-bin` ayağa kalkar, `/api/*` eylemleri Python'da gerçek iş yürütür,
  SSE event'leri yayınlanır; `qbittorrent_backend` sağlıklı
- Python suite baseline değişmez

---

## İlerleme

- 2026-08-12 — Alt-görev tanımlandı (Windows kapsamına alındı; Linux/ARM yokluğu 10b'yi engeller)
- 2026-08-12 — **Tamamlandı.** Protokol kararı: stdio JSON-RPC 2.0 (Rust python --bridge spawn eder).
  - Python: `qbittorrent_backend.py`'ye `--bridge` stdio JSON-RPC ucu: `_BRIDGE_METHODS`
    (ping/status/is_available/ensure_running/get_webui_url/get_password_status/change_webui_password/shutdown),
    `_bridge_reply`/`_bridge_error` (-32700/-32600/-32601/-32000), `_bridge_serve_loop`, `__main__` bloğu.
    Spawn `RGSX_HEADLESS=1` ile (import print'leri JSON satırlarını kirletmesin).
  - `manager-bridge/src/lib.rs`: `Bridge::spawn` (python+script, tokio child), satır-delimited
    JSON-RPC client (pending id→oneshot, reader task, timeout, stdout kapannca bekleyenleri error),
    typed wrappers + `BridgeError`. Echo fixture `tests/echo_bridge.py` — 5 unit test.
  - `manager-http`: `AppState.bridge: Option<Arc<Bridge>>` + `bridge_call()`; üç qbittorrent
    handler'ı placeholder→bridge (Python 1:1: `success==ready`, change-password 400, password-status
    spread). Contract testi `success==ready`'ye güncellendi.
  - `manager-bin`: bridge spawn (`RGSX_MANAGER_SCRIPT` env, varsayılan `../ports/RGSX/...`),
    AppState'e verilir; timeout 90s (ensure_running ~30s bloklar).
  - Doğrulama: `cargo test --workspace` 88 yeşil (30 core + 52 contract + 5 bridge + doctests);
    `cargo check --workspace` temiz. Canlı smoke: health RUNNING, password-status (bridge'ten
    gerçek available/url), qbittorrent/start (`success==ready==False`, url bridge'ten),
    change-password kısa→400 `password_too_short` / uzun→ok. Çocuk süreçler sıfır artık.
  - Python testleri (qbittorrent/password grubu) geçer.