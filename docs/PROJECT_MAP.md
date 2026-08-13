# RGSX Proje Haritası (Statik)

> Hızlı navigasyon için ilk bakılacak yer. Gerçek kaynak **codegraph**'tir; buradaki
> her "İlişkili/Bağımlı" sütunu codegraph_explore ile doğrulanmiştir. Harita ile
> codegraph çelişirse **HER ZAMAN codegraph'e güven** (harita yalnızca özet).
>
> Güncelleme kuralları: bkz. `AGENTS.md` → "Proje Haritası".

---

## 1. Rust workspace — `manager-rs/`

Workspace üyesi 7 crate + kök `Cargo.toml`. Tüm crate'lar `manager-core`'a veya
`manager-bridge`'e bağlıdır; `manager-bin` hepsini birleştirir.

| Ne | Nerede | İlişkili/Bağımlı (codegraph-doğrulu) |
|---|---|---|
| Workspace kökü | `manager-rs/Cargo.toml` | 7 member crate; workspace dep: tokio, axum, serde, serde_json, tracing, windows-rs, librqbit |
| `manager-core` (state machine + watchdog mantığı) | `manager-rs/manager-core/src/{state,watchdog,contract,lib}.rs` | Bağımlı: `manager-http` (AppState.manager_state), `manager-bridge`, `manager-windows`, `manager-torrent`. `ManagerState` (state.rs:23) çağıranlar: `watchdog.rs`, `manager-http/api.rs` |
| `manager-bridge` (TorrentBackend trait + Python subprocess köprüsü) | `manager-rs/manager-bridge/src/lib.rs` | Bağımlı: `manager-core`. `Bridge::spawn` (lib.rs:108) → `python <script> --bridge` (qbittorrent_backend.py). Typed metodlar (ping/status/get_app_paths/change_webui_password…) JSON-RPC'ye proxy. `TorrentBackend` trait'ini tanımlar; `LibrqbitEngine` + `Bridge` implement eder |
| `manager-http` (axum /api/* + SSE) | `manager-rs/manager-http/src/{api,state,sse,lib}.rs` | Bağımlı: `manager-core` (ManagerState), `manager-bridge` (TorrentBackend). `AppState` (state.rs:86) 32 çağıran: lib.rs, sse.rs, api.rs, manager-bin/main.rs. `finalize_download_in_state` (api.rs:647) → download handler'ı arka plan task'ında çağırır |
| `manager-torrent` (librqbit embedded engine) | `manager-rs/manager-torrent/src/lib.rs` + `examples/live_torrent.rs` | Bağımlı: `librqbit 8.1.1`, `manager-bridge` (impl `TorrentBackend`), tokio/serde/tracing. `LibrqbitEngine::download_torrent` (lib.rs) → `download_torrent_source` (AddTorrent + wait_until_completed + resolve_downloaded_file + link_or_copy) |
| `manager-scan` (HDD tarama + gamelist.xml + history) | `manager-rs/manager-scan/src/{scan,disk,gamelist,history}.rs` | Faz 12d — `ROMS_FOLDER` walkdir tarama (platform gruplu ROM listesi + boyut), `sysinfo` disk kullanımı, `quick-xml` ile `gamelist.xml` oku/yaz (Linux=yalnız RGSX entry, Windows=merge), `history_matches.py` portu. `manager-http` `/api/scan` (`RGSX_ROMS_FOLDER`) + SSE `scan` olayı. 8 test yeşil. |
| `manager-tvui` (TVUI native shell) | `manager-rs/manager-tvui/src/{lib,main}.rs` | Faz 12b — WebUI SPA'sını `?mode=tv` ile kiosk tarayıcıda (`http://127.0.0.1:<port>/?mode=tv`) tam ekran açar. `RGSX_TVUI=1` → `manager-bin` ayrı thread. SPA TV modu: `webui/src/App.vue` (10-foot layout + gamepad/ok nav). NOT: `wry`+`tao` webview feature bu ortamda gdk-3 çakışması nedeniyle devre dışı; harici tarayıcı varsayılan. |
| `manager-download` (DDL/debrid resolver) | `manager-rs/manager-download/src/lib.rs` | Faz 12e — `Resolver` trait + `DirectResolver` (torrent/DDL sınıflandırma) + `OneFichierResolver`/`RealDebridResolver` (kimlik gerektirir; `NotConfigured`/`NotImplemented`). `manager-http` `/api/download` DDL dalı (`RGSX_NATIVE_DOWNLOAD=1`) → `DownloadManager::resolve` → `DirectHttp` ise reqwest ile indirir, SSE/progress ile sonuçlanır. 3 test yeşil. |
| `manager-windows` (tray/autostart/firewall, cfg(windows)) | `manager-rs/manager-windows/src/{lib,tray,firewall,autostart}.rs` | Bağımlı: `manager-core`, `windows-rs`. Yalnız Windows build'de (`manager-bin` cfg(windows) dalı) linklenir; Linux'ta stub (`manager_windows_tray` modülü) |
| `manager-bin` (entrypoint + engine seçimi) | `manager-rs/manager-bin/src/main.rs` | Bağımlı: `manager-core`, `manager-http`, `manager-bridge`, `manager-torrent`, (cfg windows) `manager-windows`. `resolve_engine` (main.rs:46): `RGSX_TORRENT_ENGINE=librqbit` → `LibrqbitEngine`; aksi → `Bridge::spawn`. `AppState.bridge`'e yazar; `axum::serve` ile dinler (port 5010 / `RGSX_MANAGER_BIN_PORT`) |

### Rust↔Python köprüsü (Faz 10b, TASK-002f/002g)
- `manager-bin` `RGSX_TORRENT_ENGINE=librqbit` → `LibrqbitEngine` in-process (Python'sız).
- Varsayılan → `Bridge::spawn("qbittorrent_backend.py --bridge")` stdio JSON-RPC subprocess.
- Handler'lar `AppState.bridge: Option<Arc<dyn TorrentBackend>>` üzerinden统一 çalışır; contract değişmez.

### Rust sidecar süpervizörü (Faz 10c/1, TASK-002i)
- `ports/RGSX/rust_daemon.py` (YENİ): `rgsx_manager.main()` içinden flag-gated başlatılır. `RGSX_RUST_DAEMON=1` (ve binary mevcutsa) `manager-bin`'i subprocess sidecar olarak spawn eder (port 5010 / `RGSX_MANAGER_BIN_PORT`, engine default `librqbit`); ayrı bir daemon thread'te `/api/health` poll edip `watchdog.RestartLimiter` ile sınırlı yeniden başlatır. Durum `config.rust_daemon_available` üzerinden yansıtılır (flag kapalıysa veya binary yoksa no-op → Python-only akış korunur). Test: `tests/test_rust_daemon.py`.

### Rust torrent devri (Faz 10c/2, TASK-002j)
- `RGSX_RUST_TORRENT=1` (ayrı opt-in) + `config.rust_daemon_available` (healthy) → `network/queue.py::download_rom` torrent dalı (`torrent_meta is not None`), torrent indirmeyi `http://127.0.0.1:5010/api/download`'a devreder (gövdede `dest_path` verilir → postprocess bozulmaz). İlerleme `/api/progress` poll edilip `config.download_progress`/`config.history`'ye yansıtılır; `cancel_ev` ile iptal. Hata/timeout/`RGSX_RUST_TORRENT` kapalı → mevcut `qbittorrent_backend` yoluna **fallback** (risk sıfır, varsayılan kapalı).
- Rust tarafı: `manager-http/src/api.rs::download` artık isteğe bağlı `dest_path` kabul eder (geriye uyumlu; yoksa `dest_path_for` ile türetir). `start()` `RGSX_DOWNLOADS_FOLDER`'ı `config.ROMS_FOLDER`'a çeker.
- Yardımcı: `rust_daemon.download_torrent(...)` + `RustDaemonError`. Test: `tests/test_rust_daemon.py` (delege bayrağı + devir/mirror/fallback/missing-source).

### Rust katalog proxy (Faz 10c/3/2, TASK-002k-2) — strangler/proxy
- `manager-http/src/catalog.rs`: `CatalogSource` trait + `PythonCatalog` (reqwest/rustls-tls; `RGSX_PYTHON_MANAGER_URL`). `AppState.catalog` alanı eklendi.

### Catalog native port (Faz 12c, TASK-002o) — Python'sız catalog
- `NativeCatalog` (`catalog.rs`) `CatalogSource` implement eder: `systems_list.json` +
  `games/<platform>.json` + `languages/<lang>.json` + `images/<platform>.*` local
  dosyalarından birebir aynı JSON şeklini üretir (offline). `RGSX_NATIVE_CATALOG=1`
  → main.rs `NativeCatalog::from_env()` kurar (yollar `RGSX_DATA_DIR` altından;
  tek tek `RGSX_SOURCES_FILE`/`RGSX_GAMES_FOLDER`/`RGSX_IMAGES_FOLDER`/`RGSX_LANGUAGES_FOLDER`
  override). Komut POST'ları (download/queue) native değildir → `NativeCatalog`
  içindeki opsiyonel `PythonCatalog` fallback'e proxy edilir. Ayrı crate yerine
  `manager-http` içinde (trait zaten orada; döngü yok). 5 birim test + 102 contract yeşil.
- `platforms`/`search`/`games`/`translations`/`image` handler'ları `state.catalog` varsa Python'a proxy'ler (yanıt birebir iletilir), yoksa mevcut placeholder'a düşer (geriye uyumlu). Native Rust logic portu ileride ayrı alt faz.
- `cargo test -p manager-http`: 74/74 yeşil (6 yeni proxy testi, `FakeCatalog` ile).

### Rust durum/settings proxy (Faz 10c/3/3, TASK-002k-3) — strangler/proxy
- `settings_get`/`settings_post`/`save_filters`/`system_info`/`game_status`/`browse-directories`
  handler'ları `state.catalog` varsa Python'a proxy'lenir (GET `get_json`, POST `post_json`);
  yoksa placeholder (geriye uyumlu). `CatalogSource` trait'ine `post_json` eklendi.
- `cargo test -p manager-http`: 81/81 yeşil (7 yeni proxy testi). `system_info` birebir contract
  korunur (catalog=None → placeholder; contract testi yeşil).

### Rust destek/queue proxy (Faz 10c/3/4, TASK-002k-4) — strangler/proxy
- `cancel`/`queue`(post)/`queue/clear`/`queue/remove`/`clear-history`/`restart`/`shutdown`/
  `pause`/`resume`/`support`(zip binary)/`download/batch` handler'ları `state.catalog` varsa
  Python'a proxy'lenir (GET `get_json`, POST `post_json`/`post_binary`); yoksa placeholder/yerel.
  `CatalogSource` trait'ine `post_binary` eklendi. `/api/download/batch` route'u Rust'e eklendi.
- `cargo test -p manager-http`: 94/94 yeşil (13 yeni proxy testi).

### Rust qBittorrent bridge (Faz 10c/3/5, TASK-002k-5) — köprü trait
- `change_password`/`qb_start`/`qb_password_status` handler'ları zaten `state.bridge_call` ile
  `TorrentBackend` trait'ine bağlı (Python bridge veya librqbit); köprü yoksa placeholder.
  (Kod önceden hazırdı; 4 yeni kontrat testi eklendi — 98/98 yeşil.)
- `regenerate-password` route'u Rust'te henüz YOK (Python tarafında kalan gap).
  **KAPATILDI:** `qb_regenerate_password` handler + `regenerate_qbittorrent_password` bridge
  metodu eklendi; köprü yoksa 500 `{"success":false,"message":"bridge_unavailable"}`
  (Python sözleşmesi korunur). 99/99 test yeşil.

### Rust WebUI + SSE cutover (Faz 10c/3/6, TASK-002k-6) — flag-gated
- Rust `index`/`static_file` (static_root + hydration + traversal koruması) ve SSE `/api/events`
  (`sse.rs`, native) zaten mevcut ve testli. `RGSX_WEBUI_DIR` ile statik kök override.

### WebUI frontend (Faz 12a, TASK-002n) — native Vue 3 SPA
- `webui/` (Vite + Vue 3): `npm run build` → `webui/dist/`. Rust `tower-http::ServeDir`
  ile `/static/*` sunulur; `/` ve SPA route'ları (`/settings`,`/downloads`,...) hydrate
  edilmiş `index.html` döndürür (client-side routing, `lib.rs` fallback = `api::index`).
  Canlı ilerleme `EventSource('/api/events')` ile (SSE, TASK-002m). `RGSX_WEBUI_DIR=webui/dist`
  + `RGSX_RUST_WEBUI=1` ile aktif. 102/102 contract testi yeşil.
- `RGSX_RUST_WEBUI=1` → Rust 5000 (TV UI portu değişmez), Python SADECE `RGSX_CATALOG_PORT`
  (vars. 5001) üzerinden catalog servis eder; Rust oraya proxy'ler. `manager-bin/main.rs`
  port default 5000, `rgsx_manager.py` run_server portu 5001 + env'ler set edilir.
- Varsayılan (flag kapalı) davranış birebir korunur; launcher değişikliği gerekmedi.

---

## 2. Python network paketi — `ports/RGSX/network/`

`queue.py` merkezi worker; diğer modüller lazy import ile döngü kırar. Modül-seviyesi
state (`progress_queues`, `cancel_events`, `urls_in_progress` …) `network/__init__.py`'de
aynı obje kimliğiyle tutulur.

| Ne | Nerede | İlişkili/Bağımlı (codegraph-doğrulu) |
|---|---|---|
| `network/__init__.py` (modül-state + re-export) | `ports/RGSX/network/__init__.py` | Tüm alt modüllerin paylaştığı global state objeleri burada; `download_queue_worker`, `download_rom`, `DownloadJob` buradan çağrılır |
| `network/queue.py` (kuyruk worker + download_rom) | `ports/RGSX/network/queue.py` | `download_queue_worker` (queue.py:91) çağıranlar: `__init__.py`, `rgsx_manager.py`. `download_rom` (queue.py:629) çağıranlar: `rgsx_cli.py`, `controls/downloads.py`, `__init__.py` +2. İçe aktarır: config, qbittorrent_backend, history, display, language, utils, network.*, download_state, helpers, http_download, lolroms, archive_org, updates. `one_fichier` LAZY import (döngü kırma) |
| `network/helpers.py` (history/postprocess/torrent yardımcıları) | `ports/RGSX/network/helpers.py` | `_download_torrent_manifest_to_file` (helpers.py:188) → LAZY `network.http_download._build_browser_download_headers` (döngü kırma). `_should_prefer_qbittorrent_backend` → `qbittorrent_backend.is_available()`. `_postprocess_downloaded_file` → extract/handle_ps3 |
| `network/http_download.py` (HTTP resume/vimm/browser) | `ports/RGSX/network/http_download.py` | `queue.py`/`helpers.py` tarafından kullanılır; `_stream_response_to_path` indirme çekirdeği |
| `network/one_fichier.py` (1fichier async) | `ports/RGSX/network/one_fichier.py` | `queue.py` LAZY import eder (`is_1fichier_url`/`download_from_1fichier`); `download_state` (DownloadJob/DownloadState) kullanır |
| `network/lolroms.py`, `network/archive_org.py` | `ports/RGSX/network/{lolroms,archive_org}.py` | `queue.py` içe aktarır (URL tipi algılama/alternatif URL) |
| `network/upnp.py` (UPnP/aria2/seed status) | `ports/RGSX/network/upnp.py` | `_update_seeding_status` (upnp.py:274) → `config.history` günceller; `qbittorrent_backend.BackendUnavailableError`'a bağımlı (dead chain: yalnız qbittorrent_backend._update_seeding_status çağırır) |
| `network/updates.py` | `ports/RGSX/network/updates.py` | `queue.py` → `_safe_remove_file` |
| `network/download_state.py` (Faz 8 state machine) | `ports/RGSX/network/download_state.py` | `DownloadState` (30 satır), `DownloadEvent` (45), `DownloadJob` (260). Çağıranlar: `__init__.py`, `queue.py`, `one_fichier.py`. Test: `tests/test_download_state.py` |

---

## 3. Görev kanbanı — `tasks/`

| Ne | Nerede | İlişkili/Bağımlı |
|---|---|---|
| Şablon | `tasks/_template.md` | `environment: linux\|windows\|both` zorunlu (AGENTS.md kuralı) |
| Tamamlanan görevler | `tasks/done/*.md` | TASK-001 (Faz 7), TASK-002 + 002a–002g (Faz 10 Rust), TASK-003 (Faz 11). Hepsi `done/` |
| Aktif / bekleyen | `tasks/in-progress/`, `tasks/todo/` | 10c/2 `in-progress/TASK-002j`; 10c/3 planı + 7 alt görev `todo/TASK-002k*` |

---

## 4. Test paketi — `tests/`

| Ne | Nerede | İlişkili/Bağımlı (hedef modül) |
|---|---|---|
| Watchdog birim | `tests/test_watchdog.py` | `ports/RGSX/watchdog.py` (HysteresisMonitor/RestartLimiter) |
| qBittorrent port | `tests/test_qbittorrent_port.py` | `ports/RGSX/qbittorrent_backend.py` |
| Şifre migration | `tests/test_password_migration.py` | `ports/RGSX/qbittorrent_backend.py` (Faz 5) |
| Support ZIP | `tests/test_support_zip.py` | `utils.generate_support_zip` (Faz 1) |
| Oyun filtreleri | `tests/test_game_filters.py` | `controls/` filtre mantığı |
| Thread safety | `tests/test_thread_safety.py` | `thread_safety.py` |
| Download state | `tests/test_download_state.py` | `network/download_state.py` (Faz 8) |
| Toplu indirme | `tests/test_download_batch.py` | `controls/downloads.py` + `rgsx_web/handlers_download.py` (Faz 9) |
| Manager | `tests/test_rgsx_manager.py` | `rgsx_manager.py` (main/restart) |
| Settings | `tests/test_rgsx_settings.py` | `rgsx_settings.py` |
| qBittorrent backend | `tests/test_qbittorrent_backend.py` | `qbittorrent_backend.py` |
| API contract | `tests/test_api_contract.py` | `rgsx_web/handlers*.py` (Python REST/SSE) |
| Dil | `tests/test_language.py` | `language.py` (Faz 11) |
| Display paketi | `tests/test_display_{core,filters,helpers,exports,colors}.py` | `display/` paketi (pygame-stub; dev makinesinde gerçek pygame) |
| History noise | `tests/test_history_noise.py` | `history.py` |
| **Rust contract** | `manager-rs/manager-http/tests/contract.rs` | `manager-http` axum router (AppState, finalize_download_in_state, dest_path_for, bridge mock) — 68 test |
| **Rust birim** | `manager-rs/manager-*/tests/`, `*-rs/manager-core/src/*` `#[cfg(test)]` | crate-içi (core 30, bridge 5, torrent 9, doctest) — workspace 114 test |

---

## 5. Dokümantasyon — `docs/`

ADR dizini yok; mimari/akış/roadmap ayrı klasörlerde.

| Ne | Nerede | Kapsam |
|---|---|---|
| İndeks | `docs/README.md` | Tüm docs ağacı |
| Mimari | `docs/architecture/{NETWORK_PACKAGE,DOWNLOAD_STATE_MACHINE,WEBUI_API,CONTROLS_PACKAGE,UTILS_PACKAGE,CONCURRENCY,DOWNLOAD_MANAGER,DISPLAY_PACKAGE}.md` | Paket/katman haritaları |
| Akışlar | `docs/flows/{DOWNLOAD_PIPELINE,FILTER_PIPELINE,STARTUP,QBITTORRENT_PASSWORD}.md` | Uçtan uca akışlar |
| Roadmap | `docs/roadmap/{ROADMAP,ROADMAP_DOWNLOAD_MANAGER}.md` | Faz planı; Faz 10b + librqbit opt-in burada |
| Rehber | `docs/guides/{TESTING,DEVELOPMENT}.md` | Test / geliştirme kuralları |
| Özellikler | `docs/features/FEATURES.md` | Değişiklik günlüğü (Faz 10/10b girişleri) |
| Kullanıcı | `docs/user/{TVUI_FILTERS,WEBUI_FILTERS}.md` | UI kılavuzu |
| Eski (deprecated) | `docs/deprecated/{FOCUS_FIX,ES_INTEGRATION_ANALYSIS}.md` | Arşiv |

---

## 6. Çapraz katman bağımlılık özeti

- **Rust→Rust:** bin → {core, http, bridge, torrent, windows(cfg)}; http → {core, bridge}; torrent → {bridge, librqbit}; windows → core; bridge → core.
- **Rust→Python köprü:** `manager-bin` `Bridge::spawn` → `qbittorrent_backend.py --bridge` (stdio JSON-RPC). `LibrqbitEngine` bu yolu atlar (in-process).
- **Python→Python:** `network/*` paketi `config`/`qbittorrent_backend`/`history`/`display`/`utils`/`controls`/`rgsx_manager`'a bağlı; döngüler lazy import ile kırılır.
- **Test→Hedef:** `tests/*.py` ilgili `ports/RGSX/*` modülünü; `manager-rs/.../tests/contract.rs` + crate birim testleri Rust tarafını hedefler.
