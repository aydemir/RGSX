# network/ Paketi Mimarisi

> Faz 6-2 (commit `5aec589`): eski `network.py` (5667 satır, 94 fonksiyon) display.py
> deseniyle (8f094aa) pakete bölündü. Davranış değişmez; import yüzeyi ve modül-seviyesi
> state birebir korunur. Satır referansları commit `c5c5685` (Faz 9) itibarıyla geçerlidir.

## Özet

`network/` paketi RGSX'in indirme motorudur: kuyruk işçisi, HTTP direkt indirme (resume),
1Fichier/provider akışı, torrent indirme, Faz 8 state machine ve Faz 9 toplu indirmenin
çekirdeğidir. Hem manager süreci hem TVUI fallback modu bu paketi kullanır.

```
rgsx_manager.py ── download_queue_worker (daemon thread) ──┐
controls/downloads.py ── start_or_queue_download ──┬───────┤
rgsx_web/handlers_download.py ── _process_queued_download ─┘
                                                          │
                                  slot boşsa: thread ── asyncio.run(download_rom)
                                       ├── HTTP: http_download.py (_stream_response_to_path)
                                       ├── 1Fichier: one_fichier.py
                                       └── Torrent: qbittorrent_backend.py (helpers.py üzerinden)
                                                          │
                                        _finalize_download_result (Faz 8)
                                       ├── COMPLETED        → history Download_OK
                                       ├── RETRY_SCHEDULED  → _schedule_download_retry (backoff)
                                       └── FAILED_PERMANENT → history Erreur
```

## Modül haritası (10 modül + `__init__.py`)

| Modül | Satır | Rol |
|---|---|---|
| `queue.py` | 1770 | Kuyruk işçisi (`download_queue_worker`), `download_rom`, finalize/retry, pause/cancel/resume/shutdown |
| `one_fichier.py` | 1841 | 1Fichier gratuit + provider (AllDebrid/DebridLink/RealDebrid/TorBox) akışı |
| `updates.py` | 688 | Uygulama güncelleme kontrolü + Windows update ağacı |
| `lolroms.py` | 516 | LOLRoms özel indirme (harici araç, challenge, archive imza) |
| `download_state.py` | 415 | Faz 8 state machine: `DownloadState`/`DownloadEvent`/`transition()` + `DownloadJob` |
| `helpers.py` | 330 | Disk kontrolü, postprocess (ps3/extract), torrent helper, history feedback |
| `http_download.py` | 321 | HTTP indirme: header/challenge/resume/vimm/browser + `_stream_response_to_path` |
| `upnp.py` | 300 | UPnP port açma, aria2 torrent, seeding status (ölü zincir) |
| `archive_org.py` | 99 | archive.org URL normalizasyonu + alternatif URL denemesi |
| `__init__.py` | 174 | Modül-seviyesi state + tüm isimlerin re-export'u |

## Modül-seviyesi state (kimlik korunur)

Eski monolitin global sözlükleri `network/__init__.py`'de **aynı obje kimliğiyle** tutulur.
`thread_safety.py` (`from network import pause_events`), `rgsx_cli.py`, `controls/` aynı
objeleri görür — paketleme sırasında kimlik bozulmadı.

| State | Tip | Amaç |
|---|---|---|
| `progress_queues` | `dict` | `{task_id: queue.Queue}` — ilerleme mesajları |
| `cancel_events` | `dict` | `{task_id: threading.Event}` — iptal isteği |
| `pause_events` | `dict` | `{task_id: threading.Event}` — set = duraklatılmış |
| `download_threads` | `dict` | `{task_id: threading.Thread}` — çalışan indirme thread'leri |
| `torrent_temp_roots` | `dict` | `{task_id: temp_root}` — torrent geçici kökleri |
| `_app_shutting_down` | `bool` | Temiz kapanış bayrağı (retry/thread atlama) |
| `urls_in_progress` | `set` | İnen URL'ler (duplikasyon koruması) |
| `urls_lock` | `Lock` | `urls_in_progress` kilidi |
| `url_results` | `dict` | `{url: (success, message)}` — duplikasyon önbelleği |
| `url_done_events` | `dict` | `{url: threading.Event}` — duplikasyon senkronizasyonu |

## Kuyruk işçisi ve tek tüketici kuralı

**`download_queue_worker`** (`queue.py:91`): manager `main()` tarafından daemon thread
olarak başlatılır. 1 sn'de bir `config.download_queue`'yu poll eder; `active_download_count <
max_simultaneous_downloads` ve kuyruk doluysa ilk öğeyi pop eder, slot +1, URL tipine göre
`download_from_1fichier` veya `download_rom`'u ayrı thread'de `asyncio.run` ile başlatır.

**Tek tüketici kuralı (Faz 9):** `config.queue_worker_running` **True** ise (manager
süreci) kuyruğun tek tüketicisi `download_queue_worker`'dır — web endpoint'leri yalnızca
kuyruğa basar, legacy thread zincirini **başlatmaz** (`rgsx_manager.py` `/api/download/batch`
yönlendirmesi; `handlers_download.py` `_kick_batch_if_no_worker` :628). **False** ise
(standalone web / fallback) legacy zincir `_process_queued_download` boş slot sayısı kadar
thread başlatır. Bu kural worker + legacy zincirin aynı anda kuyruktan çift pop etmesini
önler.

## İndirme akışı

### `download_rom` (`queue.py:629`)

1. `rgsx+torrent://` / torrent URL çözümleme (`parse_torrent_download_url`, `utils/`).
2. **Duplikasyon koruması** (`urls_in_progress` + `urls_lock`): URL zaten iniyorsa
   `url_done_events[url]` Event'ine bekler, `url_results[url]`'den sonucu döner.
3. `progress_queues[task_id]` + `cancel_events[task_id]` oluşturulur.
4. Platform klasörü çözülür (`get_platform_custom_path` → `config.platform_dicts` → symlink).
5. Torrent meta varsa qBittorrent akışı, yoksa HTTP akışı.
6. Bitişte `_finalize_download_result` (`queue.py:468`) çağrılır.

### HTTP resume (`http_download.py:200-224`)

| Fonksiyon | Rol |
|---|---|
| `_http_part_path(dest)` | `.part` dosya yolu (`f"{dest}.part"`) |
| `_http_resume_offset(dest)` | Mevcut `.part` boyutu (byte), yoksa 0 |
| `_http_parse_content_range(h)` | `bytes a-b/total` → total boyut |
| `_stream_response_to_path(...)` | Akışı yazar, resume/progress yönetir |

`.part` varlığı doğal resume anahtarıdır; `Range: bytes=N-` ile devam edilir, sunucu 200
dönerse (Range desteklemiyor) baştan iner. Detay: `docs/flows/DOWNLOAD_PIPELINE.md`.

### 1Fichier (`one_fichier.py:451`)

API anahtarları varsa provider akışı, yoksa gratuit mod (wait regex, kap slotu, upgrade
önerisi). `is_1fichier_url` URL tipini belirler.

## Döngü kırma (lazy import)

`queue.py` ↔ `one_fichier.py` / `http_download.py` arasındaki çevrimsel bağımlılıklar
lazy import ile kırılır (`download_queue_worker` içinde `from network.one_fichier import
download_from_1fichier, is_1fichier_url`; aynısı `_schedule_download_retry` içinde).
`utils` 6-1 deseni ile aynı.

## Faz 8 entegrasyonu

`_finalize_download_result` sonucu `DownloadJob` state modeline geçirir:
- başarı → `COMPLETED`, history `Download_OK`, `mark_game_as_downloaded`.
- transient hata + retry hakkı varsa → `FAILED_TRANSIENT → RETRY_SCHEDULED`, history
  `Téléchargement`'te kalır (aktif görünüm), `_schedule_download_retry` backoff sonrası
  aynı URL'yi yeni task_id ile yeniden indirir (`_retry_in_flight` duplikasyonu önler,
  `_app_shutting_down`/iptal kontrolü yapar, slot kapasitesini bekler).
- kalıcı hata veya retry tükendi → `FAILED_PERMANENT`, history `Erreur`.

State makinesi tam referansı: [`DOWNLOAD_STATE_MACHINE.md`](DOWNLOAD_STATE_MACHINE.md).

## Thread güvenliği

Paket state'i `thread_safety.py` kilitle sını: `network_lock()` (master RLock) +
per-dict context manager'lar (`pause_events_lock`, `url_results_lock`, ...) + convenience
fonksiyonlar (`get_pause_event`, `request_cancel_task`, `register_download_thread`).
Detay: [`CONCURRENCY.md`](CONCURRENCY.md).

## İlgili dosyalar

- `network/queue.py` — worker, `download_rom`, finalize/retry, pause/cancel/resume/shutdown
- `network/download_state.py` — Faz 8 state machine + `DownloadJob`
- `network/http_download.py` — HTTP akış + resume
- `network/one_fichier.py` — 1Fichier/provider akışı
- `network/helpers.py` — disk, postprocess, torrent helper
- `thread_safety.py` — kilitler
- `rgsx_web/handlers_download.py` — `/api/download`, `/api/download/batch`
- `controls/downloads.py` — TVUI tarafı (`start_or_queue_download`, `queue_download_batch`)
