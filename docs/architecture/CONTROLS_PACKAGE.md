# Kontrol Paketi Referansı (`controls/`)

> Faz 6-4: eski tek parça kontrol mantığı modüllere bölündü; `__init__.py` kamu yüzeyini
> re-export eder. Satır referansları commit `c5c5685` (Faz 9) itibarıyla geçerlidir.

## Modül haritası

| Modül | Satır | Rol |
|---|---|---|
| `handlers.py` | 3936 | `handle_controls` — tek giriş noktası, durum-makinesi yönlendirme |
| `search.py` | 379 | Global arama, klavye düzeni, sonuçtan indirme tetikleme |
| `input.py` | 369 | Tuş/joystick durumu, tekrar (repeat), `controls.json` yükleme, acil durum eşlemesi |
| `menus.py` | 233 | `VALID_STATES`, `validate_menu_state`, birleşik filtre menüsü, folder browser scroll |
| `downloads.py` | 299 | Kuyruk yönetimi, batch indirme, manager delegasyonu |

## `handle_controls` — tek giriş noktası (handlers.py:276)

Dönüş: `'quit'`, `'download'`, `'redownload'` veya `None`.
Akış: `previous_menu_state` doğrulama → genel debounce (`config.debounce_delay`) →
dört giriş kaynağı (KEYDOWN/JOYBUTTONDOWN/JOYAXISMOTION/JOYHATMOTION/MOUSEBUTTONDOWN) →
mevcut `config.menu_state`'e göre dal. Kritik davranışlar:

- **`validate_menu_state`** her olaydan önce `previous_menu_state`'i kilitler (geçersiz → `platform`).
- JOYHATMOTION `(0,0)` (bırakma) `update_key_state(action, False)` ile tuşları serbest bırakır.
- `start` (pause) menüsü: `menu_state not in ("pause_menu", "controls_mapping", "reload_games_data")` koşuluyla korunur.
- `error` durumunda sadece `confirm` çıkış yapar.

## Giriş durumu ve tekrar (`input.py`)

- `key_states = {}` (input.py:13) — aksiyon → `{pressed, first_press_time, last_repeat_time, event_type, event_value}`.
- `update_key_state(action, pressed, ...)` (input.py:264) — basma durumunu kaydeder.
- `process_key_repeats(sources, joystick, screen)` (input.py:299) — `REPEAT_DELAY`/`REPEAT_INTERVAL` sonrası
  sentetik `pygame.event.Event` üretip `handle_controls`'a geri besler. **Lazy import** ile
  `controls.input ↔ controls.handlers` döngüsünü kırar.
- Joystick yoksa joystick kaynaklı `key_states` purged edilir (Bluetooth kopması → hayalet olay koruması).
- `clear_joystick_repeat_states()` (input.py:283) — manette durumlarını temizler.
- `is_input_matched` / `is_global_search_input_matched` — `controls.json` eşlemesi + alias çözümü.
- `get_emergency_controls()` (input.py:351) — yapılandırma bozuksa klavye navigasyonu için yedek eşleme.
- `load_controls_config` — `controls.json` okur; `delete_history`/`progress` alias'larını
  `clear_history`'ye normalleştirir ve diske geri yazar.

## Durum doğrulama (`menus.py`)

- `VALID_STATES` (menus.py:188) — ~40 ekran durumu: ana ekranlar, pause alt-menüleri
  (hierarchical refonte), history alt-menüleri, gelişmiş filtre menüleri, `folder_browser`.
- `validate_menu_state(state)` (menus.py:226) — falsy/geçersiz → `"platform"`.
- `open_unified_filter_menu(source_state)` (menus.py:176) — `filter_menu_choice` menüsünü açar;
  context `'game'` vs `'global'` içerik farklıdır; Faz 9'da `download_all_focus = False`.

## Kuyruk ve batch indirme (`downloads.py`)

`config.download_queue` liste; her öğe: `{url, platform, game_name, is_zip_non_supported, is_1fichier, task_id, status}`.

| Fonksiyon | Konum | Açıklama |
|---|---|---|
| `_launch_next_queued_download(force=False)` | :32 | Slot yönetimi: `max_simultaneous_downloads`'e kadar başlat; `Queued → Downloading` history güncelle |
| `_register_download_task(task_id, task, ...)` | :87 | Task kaydı + `add_done_callback` → bitişte bir sonraki kuyruk öğesini başlatır |
| `_queue_download(url, platform, game_name, ...)` | :109 | Kuyruğa ekle + history; `defer_save=True` (Faz 9) tek `save_history` |
| `queue_download_batch(games, platform_label)` | :149 | **Faz 9 TVUI**: görünen seti toplu kuyruğa alır; URL dedupe; → `(queued, skipped, already, errors)` |
| `trigger_filtered_batch_download()` | :201 | **'Tümünü İndir'**: `filtered_games`/`games` setini arka planda thread'de kuyruğa alır |
| `_delegate_download_to_manager(url, platform, game_name, ...)` | :237 | Manager modunda HTTP `POST /api/download` (port 5000) |
| `start_or_queue_download(...)` | :269 | Manager varsa delege eder; yoksa slot doluysa kuyruk, boşsa anında başlat |

Batch detayları: zaten indirilmiş oyunlar zorunlu atlanmaz (hide_downloaded filtresi kullanıcı
tercihi); yalnız `already_downloaded` sayacı sayılır. Aynı URL kuyrukta/aynı batch'te ise
tekrar eklenmez. `check_extension_before_download` başarısızsa öğe atlanır.

## Arama (`search.py`)

- `filter_games_by_search_query()` (:33) — aktif `game_filter_obj` filtresini uygular, ardından
  `config.search_query` substring eşleşmesi + `_sort_local_games` sıralama.
- `GLOBAL_SEARCH_KEYBOARD_LAYOUT` (:48) — onaylayıcı klavye ekranı.
- `trigger_global_search_download(queue_only)` (:333) — arama sonucunu indirir; desteklenmeyen
  uzantı + `allow_unknown=False` → `extension_warning` durumu.

## Kamu yüzeyi (`controls/__init__.py`)

İçe aktarma disiplini: `handlers` kullanıcıları `handle_controls`'u lazy (`tvui` içinde olay
döngüsünde) içe aktarır; `input.process_key_repeats` de kendi lazy içe aktarımını kullanır.
Döngü yalnızca `input → handlers` yönünde kırılır; `handlers` her zaman `input`'tan beslenir.

## İlgili dosyalar

- `controls/__init__.py` — re-export yüzeyi
- `controls/handlers.py` — `handle_controls` dispatcher
- `controls/input.py` — `key_states`, `process_key_repeats`, acil durum eşlemesi
- `controls/menus.py` — `VALID_STATES`, `validate_menu_state`, filtre menüsü
- `controls/downloads.py` — kuyruk/batch/manager delegasyonu
- `controls/search.py` — global arama akışı
- `controls_mapper.py` — `controls.json` düzenleyici (refonte ekranı)
