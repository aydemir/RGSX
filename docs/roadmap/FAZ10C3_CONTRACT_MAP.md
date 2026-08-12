# Faz 10c/3 — Contract Map (Rust placeholder → Python karşılığı)

> **Amaç:** Faz 10c/3 göçünde Rust `manager-http` placeholder handler'larını Python
> karşılıklarına birebir eşlemek. Altın referans: `tests/test_api_contract.py`
> (Web UI `ManagerHandler`) + `manager-rs/manager-http/tests/contract.rs` (Rust, 68 test)
> + `tests/test_rgsx_manager.py` (TV UI `RGSXHandler`).
>
> **Yöntem (2026-08-12):** `manager-rs/manager-http/src/api.rs` + `api.rs` route tablosu
> (lib.rs:31-62) ve Python dispatch (`rgsx_web/handlers.py` `do_GET`/`do_POST` 110-260,
> `rgsx_manager.py` `RGSXHandler.do_GET`/`do_POST` 180-300) codegraph ile okundu.
> Henüz okunmamış Python fonksiyon gövdeleri **DRIFT** işaretli — alt görev içinde teyit edilecek.
>
> **Durum kısaltmaları:** ✅ gerçek/fonksiyonel · ⚠️ placeholder (göç gerekir) ·
> 🔗 bridge'e bağlı · DRIFT = teyit edilmemiş.

## 1. GET route'ları

| Route | Rust handler | Durum | Python impl | Contract test |
|---|---|---|---|---|
| `/` | `index` / `placeholder_index` | ⚠️ | `handlers.py::_get_index_html` | test_api_contract (index) |
| `/static/*path` | `static_file` | ✅ | `handlers.py::_send_static` (DRIFT) | contract static testleri |
| `/api/platforms` | `platforms` | 🔗 proxy | `handlers.py::_api_platforms` | test_api_contract (**TASK-002k-2 proxy'lendi**) |
| `/api/search` | `search` | 🔗 proxy | `handlers.py::_api_search` → `controls/search.py::search_games` | test_api_contract (**TASK-002k-2 proxy'lendi**) |
| `/api/translations` | `translations` | 🔗 proxy | `handlers.py::_api_translations` → `language.py` | test_api_contract (**TASK-002k-2 proxy'lendi**) |
| `/api/games/:platform` | `games` | 🔗 proxy | `handlers.py::_api_games` → `rgsx_web/cache.py::get_cached_games` | test_api_contract (**TASK-002k-2 proxy'lendi**) |
| `/api/progress` | `progress` | ✅ | `handlers.py::_api_progress` | test_api_contract |
| `/api/game-status` | `game_status` | 🔗 proxy | `handlers.py::_api_game_status` | test_api_contract (**TASK-002k-3 proxy'lendi**) |
| `/api/history` | `history` | ✅ | `handlers.py::_api_history` | test_api_contract |
| `/api/queue` | `queue` | ✅ | `handlers.py::_api_queue_get` | test_api_contract |
| `/api/settings` | `settings_get` | 🔗 proxy | `handlers.py::_api_settings_get` → `rgsx_settings.py` | test_api_contract (**TASK-002k-3 proxy'lendi**) |
| `/api/system_info` | `system_info` | 🔗 proxy | `handlers.py::_api_system_info` / `rgsx_manager.py` system_info | **test_api_contract (birebir eşitlik — KRİTİK) (TASK-002k-3 proxy'lendi)** |
| `/api/browse-directories` | `browse_directories` | 🔗 proxy | `handlers.py::_list_directories` | test_api_contract (**TASK-002k-3 proxy'lendi**) |
| `/api/image/:platform` | `image` | 🔗 proxy | `handlers.py::_serve_platform_image` | test_api_contract (**TASK-002k-2 proxy'lendi**) |
| `/api/favicon` | `favicon` | ⚠️ | `handlers.py::_serve_favicon` | test_api_contract |
| `/api/update-cache` | `update_cache` | ⚠️ | `handlers.py::_api_update_cache` (DRIFT) | test_api_contract |
| `/api/health` | `health` | ✅ | `rgsx_manager.py` (RGSXHandler) + `handlers.py` | contract/health |
| `/api/events` (SSE) | `sse::events` | ✅ | `rgsx_manager.py::_handle_sse` | SSE testleri |

## 2. POST route'ları

| Route | Rust handler | Durum | Python impl | Contract test |
|---|---|---|---|---|
| `/api/download` | `download` | ✅ (torrent) | `handlers.py::_api_download` (HTTP) + `network/queue.py::download_rom` | contract download |
| `/api/download/batch` | `download_batch` | 🔗 proxy | `handlers.py::_api_download_batch` (Faz 9) | test_download_batch (**TASK-002k-4 eklendi+proxy**) |
| `/api/cancel` | `cancel` | 🔗 proxy | `handlers.py::_api_cancel` / `rgsx_manager.py::_handle_cancel_worker` | test_api_contract (**TASK-002k-4 proxy'lendi**) |
| `/api/queue` | `queue_post` | 🔗 proxy | `handlers.py::_api_queue_post` | test_api_contract (**TASK-002k-4 proxy'lendi**) |
| `/api/queue/clear` | `queue_clear` | 🔗 proxy | `handlers.py::_api_queue_clear` | test_api_contract (**TASK-002k-4 proxy'lendi**) |
| `/api/queue/remove` | `queue_remove` | 🔗 proxy | `handlers.py::_api_queue_remove` | test_api_contract (**TASK-002k-4 proxy'lendi**) |
| `/api/settings` | `settings_post` | 🔗 proxy | `handlers.py::_api_settings_post` → `rgsx_settings.py` | test_api_contract (**TASK-002k-3 proxy'lendi**) |
| `/api/save_filters` | `save_filters` | 🔗 proxy | `handlers.py::_api_save_filters` | test_api_contract (**TASK-002k-3 proxy'lendi**) |
| `/api/clear-history` | `clear_history` | 🔗 proxy | `handlers.py::_api_clear_history` → `history.py` | test_api_contract (**TASK-002k-4 proxy'lendi**) |
| `/api/restart` | `restart` | 🔗 proxy | `handlers.py::_api_restart` / `rgsx_manager.py` | test_api_contract (**TASK-002k-4 proxy'lendi**) |
| `/api/support` | `support` | 🔗 proxy | `handlers.py::_api_support` → `utils.generate_support_zip` | test_support_zip (**TASK-002k-4 proxy'lendi**) |
| `/api/shutdown` | `shutdown` | 🔗 proxy | `rgsx_manager.py::_trigger_shutdown` | test_rgsx_manager (**TASK-002k-4 proxy'lendi**) |
| `/api/pause` | `pause` | 🔗 proxy | `rgsx_manager.py::pause_all_downloads` | test_rgsx_manager (**TASK-002k-4 proxy'lendi**) |
| `/api/resume` | `resume` | 🔗 proxy | `rgsx_manager.py::resume_all_downloads` | test_rgsx_manager (**TASK-002k-4 proxy'lendi**) |
| `/api/qbittorrent/change-password` | `change_password` | ⚠️ (uzunluk kontrolü) | `rgsx_manager.py` → `qbittorrent_backend.change_webui_password` | test_password_migration |
| `/api/qbittorrent/regenerate-password` | — | ⚠️ yok | `rgsx_manager.py` → `qbittorrent_backend.regenerate_qbittorrent_password` | test_password_migration |
| `/api/qbittorrent/start` | `qb_start` | ⚠️ | `rgsx_manager.py` → `qbittorrent_backend.ensure_running/get_webui_url` | test_qbittorrent_*.py |
| `/api/qbittorrent/password-status` | `qb_password_status` | ⚠️ | `rgsx_manager.py` → `qbittorrent_backend.get_password_status` | test_password_migration |

## 3. Boşluk matrisi (DRIFT — TASK-002k-N içinde teyit)

- **`system_info` yanıt şekli** henüz okunmadı; `test_api_contract.py` birebir eşitlik testi
  olduğundan göçte **en riskli** alan. TASK-002k-3 önce bu yanıtı tam çıkarır.
- **`_api_update_cache`, `_serve_static`, `_api_system_info` (handlers.py)** gövdeleri bu
  keşifte okunmadı → DRIFT. TASK-002k-2/3 başında teyit edilecek.
- **RGSXHandler (TV UI) override'ları**: yalnız health/events/download/batch/cancel/shutdown/
  pause/qbittorrent*/resume override eder; geri kalan GET'ler `super().do_GET()` → `ManagerHandler`.
  Yani Web UI göçü `ManagerHandler` tabanlı; TV UI yalnız override edilenleri korur.
- **SSE olay türleri** (queue/history/progress/downloaded) Rust `sse::publish` ile zaten var;
  Python `_handle_sse` yayın şekliyle birebir uyum TASK-002k-6'da teyit edilir.

## 4. Alt görev ↔ route ataması (özet)

- **TASK-002k-2** (katalog): platforms, search, translations, games, image, update-cache
- **TASK-002k-3** (durum/settings): settings_get, settings_post, save_filters, system_info,
  browse-directories, game-status
- **TASK-002k-4** (destek/queue): support, queue_post, queue_clear, queue_remove, clear_history,
  restart, pause, resume, cancel, shutdown, download/batch
- **TASK-002k-5** (qbittorrent): change_password, qbittorrent/start, qbittorrent/password-status,
  qbittorrent/regenerate-password
- **TASK-002k-6** (Web UI+SSE): index, static_file, sse::events (cutover)
- **TASK-002k-7** (doğrulama): hepsi
