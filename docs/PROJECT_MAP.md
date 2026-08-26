# RGSX Proje Haritası (Statik)

> Hızlı navigasyon için ilk bakılacak yer. Gerçek kaynak **codegraph**'tir; buradaki
> her "İlişkili/Bağımlı" sütunu codegraph_explore ile doğrulanmıştır. Harita ile
> codegraph çelişirse **HER ZAMAN codegraph'e güven** (harita yalnızca özet).
>
> Güncelleme kuralları: bkz. `AGENTS.md` → "Proje Haritası".

---

## 0. Dal stratejisi (2026-08-22 kararı)

- **`main` = donuk Python iskelet referansı** (`ffcfcd4`, Python dönemi; manager-rs/webui yok).
  Geliştirme YOK — yalnız portlama speği olarak okunur.
- **`custom` = tek geliştirme hattı** (native Rust + ortak docs/webui/tasks).
  Yeni uzun ömürlü branch AÇILMAZ; kısa ömürlü `fix/*` serbest.
- **Cutover TAMAM (TASK-012-gap-02, 2026-08-26):** `ports/RGSX` Python uygulaması custom'dan
  silindi — custom **native-only**. Geri dönüş/nedensellik sorguları için tek kaynak
  `python-skeleton-final` tag'i (`main` BAYAT bir anlıktır, ona bakılmaz).
- Python'a acil fallback fix gerekirse: `python-skeleton-final` tag'inden kısa süreli
  branch; `custom`'a akmaz.

---

## 1. Rust workspace — `manager-rs/`

Workspace üyesi 7 crate + kök `Cargo.toml`. Tüm crate'lar `manager-core`'a veya
`manager-bridge`'e bağlıdır; `manager-bin` hepsini birleştirir.

| Ne | Nerede | İlişkili/Bağımlı (codegraph-doğrulu) |
|---|---|---|
| Workspace kökü | `manager-rs/Cargo.toml` | 7 member crate; workspace dep: tokio, axum, serde, serde_json, tracing, windows-rs, librqbit |
| `manager-core` (state machine + watchdog mantığı + settings) | `manager-rs/manager-core/src/{state,watchdog,contract,settings,lib}.rs` | Bağımlı: `manager-http` (AppState.manager_state), `manager-bridge`, `manager-windows`, `manager-torrent`. `ManagerState` (state.rs:23) çağıranlar: `watchdog.rs`, `manager-http/api.rs`. `Settings` (settings.rs) — Faz 12f native ayar şeması (`rgsx_settings.py` portu): `Default` (Python `default_settings`), `load()`/`save()` (`RGSX_SETTINGS_PATH`>`RGSX_DATA_DIR/rgsx_settings.json`), `validate()`, `system_info()`. |
| `manager-bridge` (engine-bağımsız `TorrentBackend` sözleşmesi) | `manager-rs/manager-bridge/src/lib.rs` | Bağımlı: `manager-core`. TASK-013: Python subprocess istemcisi (`Bridge`/`BridgeConfig`, `qbittorrent_backend.py --bridge`) ve qBittorrent-kavramlı default metodlar (ping/status/is_available/get_webui_url/get_password_status/change_webui_password/regenerate) söküldü. Crate yalnız sözleşmeyi taşır: `TorrentBackend` + `BridgeError` + `ProgressEvent` (+ `ExtractHint` re-export). İmplementör: `LibrqbitEngine` (manager-torrent); tüketici: manager-http `AppState.bridge_call` |
| `manager-http` (axum /api/* + SSE) | `manager-rs/manager-http/src/{api,state,sse,lib}.rs` | Bağımlı: `manager-core` (ManagerState), `manager-bridge` (TorrentBackend). `AppState` (state.rs:86) 32 çağıran: lib.rs, sse.rs, api.rs, manager-bin/main.rs. `finalize_download_in_state` (api.rs:647) → download handler'ı arka plan task'ında çağırır |
| `manager-torrent` (librqbit embedded engine) | `manager-rs/manager-torrent/src/lib.rs` + `examples/live_torrent.rs` | Bağımlı: `librqbit 8.1.1`, `manager-bridge` (impl `TorrentBackend`), tokio/serde/tracing. `LibrqbitEngine::download_torrent` (lib.rs) → `download_torrent_source` (AddTorrent + wait_until_completed + resolve_downloaded_file + link_or_copy). Gap-2: `active_handles` (task_id → handle) kaydı + `Session::pause/unpause` ile `pause_active`/`resume_active`/`pause_task`/`resume_task`; `call()` JSON-RPC `pause_all`/`resume_all`/`pause`/`resume`/`is_paused`. Gap-3: `cancel_task`/`cancel_all_tasks` (`Session::delete(delete_files=true)` — `.rqbitpart`/kısmi dosyaları siler), progress loop'u `active_handles`'tan düşen task'ı iptal olarak görür; `call()` JSON-RPC `cancel`/`cancel_all` |
| `manager-scan` (HDD tarama + gamelist.xml + history) | `manager-rs/manager-scan/src/{scan,disk,gamelist,history}.rs` | Faz 12d — `ROMS_FOLDER` walkdir tarama (platform gruplu ROM listesi + boyut), `sysinfo` disk kullanımı, `quick-xml` ile `gamelist.xml` oku/yaz (Linux=yalnız RGSX entry, Windows=merge), `history_matches.py` portu. `manager-http` `/api/scan` (`RGSX_ROMS_FOLDER`) + SSE `scan` olayı. 8 test yeşil. |
| `manager-tvui` (TVUI native shell) | `manager-rs/manager-tvui/src/{lib,main,sdl2_shell,theme,render,screens,state,menus,search,virtual_keyboard,folder_browser,accessibility}.rs` + `native_input.rs` | Faz 12b (yön B) — `rust-sdl2` ile `display/*`+`controls/*` pygame `draw_*`'larının native 10-foot render portu. `theme.json` serde_json ile yüklenir. `RGSX_TVUI=1` → SDL2 native shell — tek TVUI yolu (Python fallback gap-02 ile söküldü). Girdi: `native_input.rs` (gilrs gamepad, TASK-005). |
| `manager-download` (DDL/debrid resolver) | `manager-rs/manager-download/src/lib.rs` | Faz 12e — `Resolver` trait + `DirectResolver` (torrent/DDL sınıflandırma) + `OneFichierResolver`/`RealDebridResolver` (kimlik gerektirir; `NotConfigured`/`NotImplemented`). `manager-http` `/api/download` DDL dalı (`RGSX_NATIVE_DOWNLOAD=1`) → `DownloadManager::resolve` → `DirectHttp` ise reqwest ile indirir, SSE/progress ile sonuçlanır. 3 test yeşil. |
| `manager-windows` (tray/autostart/firewall, cfg(windows)) | `manager-rs/manager-windows/src/{lib,tray,firewall,autostart}.rs` | Bağımlı: `manager-core`, `windows-rs`. Yalnız Windows build'de (`manager-bin` cfg(windows) dalı) linklenir; Linux'ta stub (`manager_windows_tray` modülü) |
| `manager-bin` (entrypoint + engine seçimi) | `manager-rs/manager-bin/src/main.rs` | Bağımlı: `manager-core`, `manager-http`, `manager-bridge`, `manager-torrent`, (cfg windows) `manager-windows`. `resolve_engine`: tek yol in-process `LibrqbitEngine` — TASK-013: Python dalı söküldü, `RGSX_TORRENT_ENGINE` env'i yok sayılır. `AppState.bridge`'e yazar; `axum::serve` ile dinler (port 5010 / `RGSX_MANAGER_BIN_PORT`) |

### Rust↔Python torrent köprüsü (Faz 10b, TASK-002f/002g) — TASK-013 ile emekli
- `manager-bin` tek torrent yolu: in-process `LibrqbitEngine` (Python'sız).
- Arşiv: eski varsayılan `Bridge::spawn("qbittorrent_backend.py --bridge")` stdio JSON-RPC
  subprocess'ı söküldü; geri dönüş için `python-skeleton-final` tag'i.
- Handler'lar `AppState.bridge: Option<Arc<dyn TorrentBackend>>` üzerinden统一 çalışır; contract değişmez.

### Rust sidecar süpervizörü (Faz 10c/1, TASK-002i) — ARŞİV (gap-02)
- `ports/RGSX/rust_daemon.py` Python portuyla silindi (python-skeleton-final tag'inde yaşar).
  Native-only'de manager-bin tek süreçtir; supervisor'a gerek yoktur.

### Rust torrent devri (Faz 10c/2, TASK-002j) — ARŞİV (gap-02)
- `network/queue.py` devri ve `rust_daemon.download_torrent(...)` yardımcısı Python portuyla
  silindi; torrent indirme yalnız in-process `LibrqbitEngine` üzerinden yürür (TASK-013).
- Kalan Rust tarafı: `manager-http/src/api.rs::download` isteğe bağlı `dest_path` kabul eder
  (geriye uyumlu; yoksa `dest_path_for` ile türetir). `start()` `RGSX_DOWNLOADS_FOLDER`'ı
  ROM köküne çeker.

### Rust katalog proxy (Faz 10c/3/2, TASK-002k-2) — SÜPERSEDED (gap-02)
- `PythonCatalog` + `RGSX_PYTHON_MANAGER_URL` proxy'si TASK-012-gap-02 ile söküldü.
  Tarihçe: `CatalogSource` trait + reqwest proxy (`AppState.catalog`) idi.

### Catalog native port (Faz 12c, TASK-002o) — tek katalog kaynağı
- `NativeCatalog` (`catalog.rs`) `CatalogSource` implement eder: `systems_list.json` +
  `games/<platform>.json` + `languages/<lang>.json` + `images/<platform>.*` local
  dosyalarından birebir aynı JSON şeklini üretir (offline). main.rs artık
  koşulsuz `NativeCatalog::from_env()` kurar (yollar `RGSX_DATA_DIR` altından;
  tek tek `RGSX_SOURCES_FILE`/`RGSX_GAMES_FOLDER`/`RGSX_IMAGES_FOLDER`/`RGSX_LANGUAGES_FOLDER`
  override; gap-02 ile `RGSX_NATIVE_CATALOG` flag'i ve Python fallback söküldü).

### Native katalog OTA bootstrap (Faz 12f, TASK-004) — veri otomatik çekme
- `manager-http/src/catalog_bootstrap.rs`: `ensure_catalog_ready()` — `RGSX_NATIVE_CATALOG=1`
  ama `RGSX_DATA_DIR`'de `systems_list.json` + `games/*.json` yoksa OTA `games.zip`'i
  indirip çıkarır (Python `rgsx_cli.ensure_data_present` birebir karşılığı, saf-Rust).
  Zip URL: `RGSX_SOURCES_MODE=custom`+`RGSX_SOURCES_ZIP_URL` / yerel `games.zip` /
  varsayılan `https://retrogamesets.fr/softs/games.zip`. `main.rs` native dalında
  `NativeCatalog::from_env()` öncesi `.await` edilir. Bağımlılık: `zip` (deflate) + reqwest `stream`.
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

### Rust qBittorrent bridge (Faz 10c/3/5, TASK-002k-5) — TASK-013 ile emekli edildi
- Arşiv: `/api/qbittorrent/*` uçları + handler'ları (`change_password`/`qb_start`/
  `qb_password_status`/`qb_regenerate_password`) + trait default'ları söküldü — librqbit tek torrent yolu.
- Yaşayan kısım: `state.bridge_call` → `TorrentBackend::call` (download/pause/resume/cancel ailesi).

### Rust WebUI + SSE cutover (Faz 10c/3/6, TASK-002k-6) — flag-gated
- Rust `index`/`static_file` (static_root + hydration + traversal koruması) ve SSE `/api/events`
  (`sse.rs`, native) zaten mevcut ve testli. `RGSX_WEBUI_DIR` ile statik kök override.

### WebUI frontend (Faz 12a, TASK-002n) — native Vue 3 SPA
- `webui/` (Vite + Vue 3): `npm run build` → `webui/dist/`. Rust `tower-http::ServeDir`
  ile `/static/*` sunulur; `/` ve SPA route'ları (`/settings`,`/downloads`,...) hydrate
  edilmiş `index.html` döndürür (client-side routing, `lib.rs` fallback = `api::index`).
  Canlı ilerleme `EventSource('/api/events')` ile (SSE, TASK-002m). `RGSX_WEBUI_DIR=webui/dist`
  + `RGSX_RUST_WEBUI=1` ile aktif. 114 contract testi yeşil.
- `RGSX_WEBUI_DIR=webui/dist` statik kök override; SPA her zaman Rust'tan servis edilir
  (gap-02: Python catalog portu 5001 ve proxy'si söküldü, tek sunucu manager-bin).

### Settings native port (Faz 12f, TASK-002s) — typed `Settings` şeması
- `manager-core/src/settings.rs`: `Settings` struct (tüm `rgsx_settings.py` `default_settings`
  alanları typed: `language: Option<String>` + `skip_serializing_if` → "key yok = seçim yok"
  kuralı korunur), `Accessibility`/`Display`/`Symlink`/`Sources` alt şemaları, `flatten extra`
  (round-trip için `game_filters` vb.), `Default` (Python birleşimi), `load()`/`save()`
  (`RGSX_SETTINGS_PATH` > `RGSX_DATA_DIR/rgsx_settings.json`), `validate()` (invariant kontrolü),
  `system_info()` (env tabanlı). `native_enabled()` = `RGSX_NATIVE_SETTINGS=1`.
- `manager-http/src/api.rs` `settings_get`/`settings_post`: `RGSX_NATIVE_SETTINGS=1` ve
  `catalog=None` → native `Settings::load()` + validasyon + `save()`; aksi → Python proxy /
  placeholder (kesintisiz göç). Geçici alanlar (`auto_extract`/`api_keys`/`web_service_at_boot`/
  `custom_dns_at_boot`) native save'de strip edilir (Option A: port sonraki faza bırakıldı).
- `cargo test -p manager-core` (settings 6 test) + `manager-http` contract
  `test_settings_native_roundtrip` yeşil; tam contract 103/103.

---

## 2. Python network paketi — `ports/RGSX/network/` — ARŞİV (gap-02)

`ports/RGSX` Python uygulaması TASK-012-gap-02 ile custom'dan silindi; bu bölüm
tarihsel referanstır. Network/queue/download-state mantığının Rust karşılıkları:
`manager-http` (kuyruk+SSE+api), `manager-download` (DDL/debrid resolver),
`manager-torrent` (librqbit engine). Tam akış haritası: `docs/PYTHON_WORKFLOW.md`
(arşiv; Rust eşleme tablosu hâlâ geçerli bir indeks).

---

## 3. Görev kanbanı — `tasks/`

| Ne | Nerede | İlişkili/Bağımlı |
|---|---|---|
| Şablon | `tasks/_template.md` | `environment: linux\|windows\|both` zorunlu (AGENTS.md kuralı) |
| Tamamlanan görevler | `tasks/done/*.md` | TASK-001 (Faz 7), TASK-002 + 002a–002g (Faz 10 Rust), TASK-003 (Faz 11). Hepsi `done/` |
| Aktif / bekleyen | `tasks/in-progress/`, `tasks/todo/` | Faz 12 çekirdeği (12a–12e + 12f) `done/`; kalan `in-progress`: 002i/002j/002k-7/002l/002m (commit'lendi, temizlenecek). `todo/TASK-002k-faz10c3-plan.md` (Faz 10c/3 planı, Faz 12 ile büyük ölçüde kapsandı) |
| Rust gap'leri (Faz 13) | `tasks/gap/TASK-002-gap-*.md`, tamamlanan `tasks/done/` | Python iş akışında (PYTHON_WORKFLOW.md) Rust karşılığı OLMAYAN düğümler: retry engine, cancel+temizlik, HTTP-direct, disk alanı, extract, seed lifecycle, stray-temp, restart-resume, history/SSE, 1fichier provider zinciri. ✅ **Gap 2 (pause/resume)** (3ba8a8e) + ✅ **Gap 3 (cancel+temizlik)** tamamlandı (`tasks/done/`). P0→P2 sıralı (Faz 13 roadmap): sıradaki Gap 4 (HTTP-direct). |

---

## 4. Test paketi — Rust (`manager-rs/`)

Kök `tests/` pytest süiti gap-02 ile silindi (karşıladığı API sözleşmeleri
`contract.rs`'te; birim davranışlar crate testlerinde). Arşiv: `python-skeleton-final`.

| Ne | Nerede | İlişkili/Bağımlı (hedef modül) |
|---|---|---|
| **Rust contract** | `manager-rs/manager-http/tests/contract.rs` | `manager-http` axum router (AppState, finalize_download_in_state, dest_path_for, bridge mock) — 105 test (TASK-013 sonrası; qbittorrent uç testleri söküldü) |
| **Rust faz5 smoke** | `manager-rs/manager-http/tests/faz5_smoke.rs` | self_update apply/rollback canlı smoke (ready timeout 180 sn) |
| **Rust birim** | `manager-rs/manager-*/tests/`, `*-rs/manager-core/src/*` `#[cfg(test)]` | crate-içi + workspace entegrasyon — `cargo test --workspace` yeşil; güncel dağılım: core 75, download 29 (+14 http_integration), http lib 28, scan 8, torrent 4 (+12 engine), tvui 27, windows 6 |

---

## 5. Dokümantasyon — `docs/`

ADR dizini yok; mimari/akış/roadmap ayrı klasörlerde.

| Ne | Nerede | Kapsam |
|---|---|---|
| İndeks | `docs/README.md` | Tüm docs ağacı |
| Mimari | `docs/architecture/{NETWORK_PACKAGE,DOWNLOAD_STATE_MACHINE,WEBUI_API,CONTROLS_PACKAGE,UTILS_PACKAGE,CONCURRENCY,DOWNLOAD_MANAGER,DISPLAY_PACKAGE}.md` | Paket/katman haritaları |
| Akışlar | `docs/flows/{DOWNLOAD_PIPELINE,FILTER_PIPELINE,STARTUP,QBITTORRENT_PASSWORD}.md` | Uçtan uca akışlar |
| Roadmap | `docs/roadmap/{ROADMAP,ROADMAP_DOWNLOAD_MANAGER,ROADMAP_FAZ12_RUST_WEBUI_TVUI,FAZ12_PARITY_STRATEGY,FAZ10C3_CONTRACT_MAP}.md` | Faz planı; Faz 10b + librqbit opt-in; Faz 13 (Rust download gap'leri) burada; parity stratejisi Faz 12 kontrollü ayrılma rehberi |
| İş akışı | `docs/PYTHON_WORKFLOW.md` (arşiv) | Python indirme akışı haritası — Rust eşleme tablosu hâlâ geçerli indeks; canlı akış manager-http/download/torrent'te |
| Rehber | `docs/guides/{TESTING,DEVELOPMENT}.md` | Test / geliştirme kuralları |
| Özellikler | `docs/features/FEATURES.md` | Değişiklik günlüğü (Faz 10/10b girişleri) |
| Kullanıcı | `docs/user/{TVUI_FILTERS,WEBUI_FILTERS}.md` | UI kılavuzu |
| Eski (deprecated) | `docs/deprecated/{FOCUS_FIX,ES_INTEGRATION_ANALYSIS}.md` | Arşiv |

---

## 6. Çapraz katman bağımlılık özeti

- **Rust→Rust:** bin → {core, http, bridge, torrent, windows(cfg)}; http → {core, bridge}; torrent → {bridge, librqbit}; windows → core; bridge → core.
- **Arşiv Rust→Python torrent köprüsü:** TASK-013 öncesi `manager-bin` `Bridge::spawn` → `qbittorrent_backend.py --bridge` (stdio JSON-RPC); bugün tek yol in-process `LibrqbitEngine`.
- **Arşiv Python→Python:** `network/*` paketi `config`/`qbittorrent_backend`/`history`/`display`/`utils`/`controls`/`rgsx_manager`'a bağlıydı; döngüler lazy import ile kırılırdı (gap-02 ile silindi).
- **Test→Hedef:** `manager-rs/.../tests/*.rs` + crate-içi `#[cfg(test)]` modülleri Rust tarafını hedefler.
