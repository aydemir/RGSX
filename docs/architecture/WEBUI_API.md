# WebUI Paketi + REST/SSE API Referansı

> Faz 6-3 (commit `d996633` dönemi): eski `rgsx_web.py` (2408 satır) pakete bölündü.
> Satır referansları commit `c5c5685` (Faz 9) itibarıyla geçerlidir.

## Paket yapısı (`rgsx_web/`, 9 modül)

| Modül | Satır | Rol |
|---|---|---|
| `handlers.py` | 265 | `RGSXHandler` — `do_GET`/`do_POST` dispatcher + yanıt yardımcıları |
| `handlers_download.py` | 796 | `/api/download`, `/api/download/batch`, progress, history, queue, cancel |
| `handlers_settings.py` | 430 | settings get/post, save_filters, system_info, update-cache, restart, support |
| `handlers_ui.py` | 353 | statik dosya, platform görseli, favicon, dizin listeleme, index HTML |
| `handlers_games.py` | 209 | platforms, search, translations, games |
| `__init__.py` | 325 | logging bootstrap, ilk veri yükleme, re-export |
| `cache.py` | 200 | ETag/Last-Modified/304, kaynak/oyun cache + watchdog invalidasyon |
| `server.py` | 134 | `run_server` + `CURRENT_HTTPD` |
| `i18n.py` | 101 | `load_translations`/`TRANSLATIONS`/`get_translation`/`normalize_size` |

## Dispatcher

`RGSXHandler(UIMixin, GamesMixin, DownloadMixin, SettingsMixin, BaseHTTPRequestHandler)`
do_GET: 18 route, do_POST: 11 route (handlers.py:97 ve :193). Bilinmeyen route → 404 JSON
(`{"success": False, "error": "Route non trouvée", "path": ...}`). Hata → 500 JSON.
ETag/`If-None-Match` ve `Last-Modified`/`If-Modified-Since` → 304 destekli (`_send_json`).

## GET endpoint'leri

| Route | Açıklama | Yanıt özeti |
|---|---|---|
| `/` `/index.html` `/platform/<p>` `/downloads` `/history` `/settings` | SPA index HTML | HTML |
| `/static/<path>` | Statik dosya (CSS/JS/resim) | dosya |
| `/api/platforms` | Platform listesi (gizli/unsupported filtresi + `games_count`) | `{success, count, platforms[]}` |
| `/api/search?q=<term>` | Evrensel arama (sistemler + oyunlar) | `{success, search_term, results:{platforms[], games[]}}` |
| `/api/translations` | Çeviri anahtarları (`_language` dahil, her istekte taze) | `{success, language, translations}` |
| `/api/games/<platform>` | Platform oyun listesi (size normalize, downloaded flag) | `{success, platform, count, games[]}` |
| `/api/progress` | Sadece devam eden indirmeler (history'den) | `{success, downloads{url: {...}}}` |
| `/api/game-status` | İndirilen/indiriliyor/başarısız özeti | `{success, statuses{stem: {status, ...}}}` |
| `/api/history` | Tüm görünür history (en yeni önce, hata mesajı sadeleştirilmiş) | `{success, count, history[]}` |
| `/api/queue` | Aktif + kuyruk durumu | `{success, active, queue[], queue_size}` |
| `/api/settings` | Settings + dinamik seçenekler (`auto_extract`, Linux boot seçenekleri, api_keys) | `{success, settings, system_info}` |
| `/api/system_info` | Batocera/sistem bilgisi | `{success, system_info}` |
| `/api/update-cache` | Cache temizle + OTA `games.zip` indir/çıkar + torrent manifest refresh | `{success, message, deleted[]}` |
| `/api/image/<platform>` | Platform görseli | image |
| `/api/favicon` | Favicon | image |
| `/api/browse-directories?path=` | Klasör listeleme (sürücü kökü desteği) | JSON |

## POST endpoint'leri

| Route | Body | Açıklama |
|---|---|---|
| `/api/download` | `{platform, game_index \| game_name, url, mode: "now"\|"queue"}` | İndirme ekle; 400: eksik/invalid parametre |
| `/api/download/batch` | `{platform, game_names[]}` | **Faz 9** toplu indirme; URL dedupe, `already_downloaded` sayacı, tek sefer `save_history` → `{queued, skipped, already_downloaded, errors}` |
| `/api/cancel` | `{task_id}` | İndirme iptal (senkronize pop, worker yaymaz) |
| `/api/queue` | `{action}` | Kuyruk yönetimi (get/post) |
| `/api/queue/clear` | — | Kuyruğu boşalt (çalışan ilk öğe hariç) |
| `/api/queue/remove` | `{task_id}` | Kuyruktan öğe sil |
| `/api/settings` | `{settings}` | Settings kaydet (`auto_extract`/`web_service_at_boot`/`custom_dns_at_boot`/`api_keys` ayrı ele alınır) |
| `/api/save_filters` | `{region_filters, hide_non_release, one_rom_per_game, hide_downloaded, regex_mode, region_priority}` | Yalnız filtreleri kaydet + `config.game_filter_obj` güncelle |
| `/api/clear-history` | — | History temizle |
| `/api/restart` | — | 2 sn sonra uygulamayı yeniden başlat |
| `/api/support` | — | Support ZIP üret (`rgsx_settings.json` redakte edilmiş) |

## Manager endpoint'leri (`rgsx_manager.py` `ManagerHandler`, port 5000)

`ManagerHandler(RGSXHandler)` super handler'ın önüne kendi route'larını ekler.

**GET:**
- `/api/health` — `{success, status, manager, version, pid, manager_state}`
- `/api/qbittorrent/password-status` — varsayılan şifre kontrolü (Faz 5)
- `/api/events` — **SSE** akışı (snapshot/progress/history/queue/downloaded + Faz 8 `download_state`)

**POST:**
- `/api/download` / `/api/download/batch` — worker'a delege (batch: **kick yok**, tek tüketici)
- `/api/cancel` — `_handle_cancel_worker`
- `/api/shutdown` — daemon'ı kapat
- `/api/pause` — `pause_all_downloads()` → `{success, paused}`
- `/api/resume` — `resume_all_downloads()` → `{success, resumed}`
- `/api/qbittorrent/start` — qBittorrent'i başlat → `{success, ready, url}`
- `/api/qbittorrent/change-password` — `{password}` → yeni WebUI şifresi
- `/api/qbittorrent/regenerate-password` — rastgele şifre üret+uygula → `{success, password}`

## SSE olayları (`/api/events`)

`_broadcaster_loop` değişiklikleri yayınlar; TVUI `_manager_sse_worker` (tvui.py:450) bunları
uygular. Faz 8 `download_state` olayları: `completed`, `retry_scheduled`, `failed_permanent`
(`network/download_state.py` `set_state_emitter(_broadcast)` ile bağlanır). TVUI eşlemesi:
`docs/flows/STARTUP.md` §5.

## Ortak davranış

- **CORS:** `Access-Control-Allow-Origin: *` (tüm yanıtlarda).
- **Dil:** cookie `language` (yoksa `en`); `/api/translations` diskten taze okur.
- **Cache:** `ETag` + `Last-Modified`; 304 yanıtları `_send_json`'da.
- **Hata yüzeyi:** 400 (parametre), 404 (route), 500 (iç hata) — hepsi JSON `{success, error}`.
- **Log:** `log_message` sessiz (verbosity); `_api_*` hataları `logger.error`.

## İlgili dosyalar

- `rgsx_web/handlers.py` — dispatcher
- `rgsx_web/handlers_download.py` — indirme/kuyruk/batch
- `rgsx_web/handlers_settings.py` — settings/support/restart
- `rgsx_web/handlers_games.py` — platform/arama/çeviri
- `rgsx_web/cache.py` — ETag/Last-Modified + invalidasyon watchdog
- `rgsx_manager.py` — `ManagerHandler` + SSE + qBittorrent route'ları
- `static/js/app.js` — WebUI frontend
