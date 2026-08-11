# Kritik Akış: İndirme Pipeline'ı (HTTP Resume + Torrent + Kuyruk)

> Geliştirici notu: Satır referansları commit `c5c5685` (Faz 9) itibarıyla geçerlidir.
> Modüller: `network/` paketi (`queue.py`, `http_download.py`, `one_fichier.py`,
> `download_state.py`), `controls/downloads.py`, `rgsx_manager.py`.

## Özet

İndirme isteği hangi istemciden gelirse gelsin (TVUI, WebUI, CLI, tray) **tek karar
noktasına** düşer. Manager aktifse HTTP ile delege edilir, değilse yerel kuyruk/thread
kullanılır. İki indirme türü vardır: **HTTP direkt** (1Fichier/AllDebrid/DebridLink/
RealDebrid/TorBox) ve **torrent** (gömülü qBittorrent). Faz 8 (durum makinesi + retry)
ve Faz 9 (toplu indirme) aynı pipeline üzerine kuruludur.

```
TVUI controls/downloads.py ── start_or_queue_download ──┐
WebUI /api/download ── manager ─────────────────────────┼──► network/queue.py
CLI rgsx_cli.py ────────────────────────────────────────┤    download_queue_worker
                                                              │
                                    (Faz 9) /api/download/batch ┘
                                                              │
                                                              ▼
                                                         slot boşsa: thread ── asyncio.run(download_rom)
                                                              ├── HTTP: http_download.py (_stream_response_to_path: .part + Range)
                                                              └── torrent: qbittorrent_backend.py API
```

## 1. İstek karar noktası: `start_or_queue_download` (controls/downloads.py:269)

1. `config.manager_available` True → `_delegate_download_to_manager()` (downloads.py:237):
   - background thread'de `POST http://127.0.0.1:{port}/api/download`
   - body: `{platform, game_name, url, mode: "now"}`
   - sonuç `("queued", "manager")` — TVUI slot saymaz, manager yönetir.
2. Manager yoksa yerel: aktif slot (`active_download_count`) ≥ `max_simultaneous_downloads`
   ise `_queue_download()` → `config.download_queue`'ya ekle, `("queued", task_id)`.
3. Slot boşsa doğrudan başlat:
   - `is_1fichier_url(url)` → `download_from_1fichier` (API anahtarı yoksa **gratuit** mod)
   - değilse → `download_rom`
   - `asyncio.create_task` + `_register_download_task(...)` (slot+1, task kaydı).

## 2. Kuyruk işçisi: `download_queue_worker` (network/queue.py:91)

Manager `main()` tarafından daemon thread olarak başlatılır (`rgsx_manager.py`).
**Tek tüketici kuralı (Faz 9):** worker çalışıyorken (`config.queue_worker_running`)
legacy thread zinciri kuyruktan pop etmez — aksi halde çift tüketim olur.

```python
while True:
    active < max_dl and config.download_queue:   # slot kontrolü
        job = config.download_queue.pop(0)
        active += 1; config.download_active = True
        # is_1fichier? → asyncio.run(download_from_1fichier(...)) : download_rom(...)
        threading.Thread(target=..., daemon=True).start()
    time.sleep(1)
```

- Slot serbest kaldığında `notify_download_finished()` (network/queue.py:118) çağrılır:
  `active_download_count -= 1`.
- Worker hata durumunda 2 sn bekleyip devam eder — kuyruk asla ölmez.

## 3. HTTP indirme + resume (network/http_download.py:200–224)

Resume mekanizması dört yardımcı fonksiyondan oluşur:

| Fonksiyon | Rol | Konum |
|---|---|---|
| `_http_part_path(dest)` | `.part` dosya yolu: `f"{dest}.part"` | http_download.py:200 |
| `_http_resume_offset(dest)` | Mevcut `.part` boyutu (byte), yoksa 0 | http_download.py:203 |
| `_http_parse_content_range(h)` | `bytes a-b/total` → total boyut | http_download.py:213 |
| `_stream_response_to_path(...)` | Akışı yazar, resume/progress yönetir | http_download.py:224 |

`_stream_response_to_path` akışı:

1. `resume_offset = _http_resume_offset(dest)`; `is_range = resume_offset>0 and status==206`.
2. **Toplam boyut:** `Content-Range` total varsa ondan; yoksa `Content-Length + resume_offset`
   (206 iken); yoksa `fallback_total_size`.
3. Dosya açılış modu: `'ab'` (206 resume) veya `'wb'` (baştan).
4. Chunk döngüsü (4096 byte):
   - `pause_events[task_id]` set ise pause (0.1 sn poll; cancel öncelikli kırar).
   - `cancel_events[task_id]` set → `download_canceled=True`, `.part` silinir.
   - İlerleme: her ≥0.1 sn veya % değişimde `progress_queue_obj.put((task_id, downloaded, total, speed))`.
5. Bitiş:
   - iptal → `.part` sil.
   - `downloaded > 0` → `os.replace(part_path, dest_path)` (atomik finalize).
   - 0 byte → `.part` sil.
6. Dönüş: `{total_size, downloaded, download_canceled, ...}`.

**Önemli:** Sunucu Range desteklemiyorsa (200 döner) eski `.part`'ı `'wb'` ile **baştan**
yazar — `resume_offset` sıfırdan sayılır. `.part` varlığı doğal resume anahtarıdır.

## 4. `download_rom` (network/queue.py:629) — torrenthandler + history + dispatcher

1. `parse_torrent_download_url(url)` → `torrent_meta` (yoksa None) — `rgsx+torrent://` için
   `utils/` paketi zorlama yolu.
2. **Duplikasyon koruması** (`urls_in_progress` + `urls_lock`): URL zaten iniyorsa
   `url_done_events[url]` Event'ine 30 dk bekler, `url_results[url]` önbelleğinden sonucu döner.
3. `progress_queues[task_id]` + `cancel_events[task_id]` oluştur.
4. `download_thread()` (iç fonksiyon):
   - `config.history = load_history()` → mevcut girişi `"Downloading"`'e resetle ya da yeni
     giriş ekle; `_save_history_with_feedback` ile diske yaz.
   - Platform özel klasör (`get_platform_custom_path`) yoksa `config.platform_dicts`'ten
     `folder` çözülür; symlink ayarı uygulanır.
   - `torrent_meta` varsa torrent akışına (qBittorrent), yoksa HTTP akışına gider.
   - Bitişte `_finalize_download_result()` (network/queue.py:468): başarı → `COMPLETED`/
     `Download_OK`; **transient hata** → `FAILED_TRANSIENT → RETRY_SCHEDULED`
     (`_schedule_download_retry`, üstel backoff); kalıcı → `FAILED_PERMANENT`/`Erreur`.
   - `notify_download_finished()` çağrılır, `url_results`/`url_done_events` temizlenir.

## 5. 1Fichier: `download_from_1fichier` (network/one_fichier.py:451)

- API anahtarları varsa provider akışı (AllDebrid/DebridLink/RealDebrid/TorBox), yoksa
  **gratuit** mod: HTTP istekler + bekletme (wait) mantığı.
- `is_1fichier_url` URL tipini belirler; `start_or_queue_download` ve
  `download_queue_worker` bu ayırımı yapar.
- Aynı Faz 8 finalize/retry akışına bağlıdır (queue.py ile lazy import).

## 6. Torrent (gömülü qBittorrent)

- qBittorrent-nox (embedded) `qbittorrent_backend.py` API ile yönetilir; WebUI 18572
  (doluysa Faz 3 fallback portu).
- Kısmi veri qBittorrent'te korunur → **restart sonrası `_resume_interrupted_downloads`
  (rgsx_manager.py)** history'den "Downloading/Paused" girdilerini kuyruğa geri ekler,
  torrent kaldığı yerden devam eder.
- TVUI yerel resume `_prewarm_qbittorrent_startup()` ile torrent girdisi varsa qB'yi önceden başlatır.

## 7. İptal / Pause / Resume

| Eylem | Mekanizma |
|---|---|
| Cancel | `cancel_events[task_id].set()` → akış iptal, `.part` sil; manager: `/api/cancel` |
| Pause | `pause_events[task_id].set()` → chunk döngüsü pause; `/api/pause` (toplu) |
| Resume | `pause_events[task_id].clear()` → kaldığından devam; `/api/resume` |
| Slot | `active_download_count`; bittiğinde `notify_download_finished()` |

## 8. Toplu indirme (Faz 9)

- **Web/manager:** `POST /api/download/batch` (`rgsx_web/handlers_download.py:501`) —
  `{platform, game_names[]}`; URL dedupe, `already_downloaded` sayacı, `save_history` tek
  sefer; `_kick_batch_if_no_worker` (handlers_download.py:628) tek tüketici kuralını uygular.
- **TVUI:** `controls/downloads.py:149 queue_download_batch` + `trigger_filtered_batch_download`
  (downloads.py:201) — daemon thread, görünen (filtrelenmiş) seti kuyruğa alır.

## İlgili dosyalar

- `network/queue.py` (worker, `download_rom`, finalize/retry, cancel/pause state'leri)
- `network/http_download.py` (HTTP akış + resume)
- `network/one_fichier.py` (1Fichier akışı)
- `network/download_state.py` (Faz 8 state machine + `classify_error` + backoff)
- `controls/downloads.py` (`start_or_queue_download`, `_delegate_download_to_manager`,
  `queue_download_batch`, `trigger_filtered_batch_download`)
- `rgsx_web/handlers_download.py` (`/api/download`, `/api/download/batch`,
  `_kick_batch_if_no_worker`)
- `rgsx_manager.py` (worker spawn, `_resume_interrupted_downloads`, `/api/*`)
- `qbittorrent_backend.py` (torrent API)
- `rgsx_settings.py` (`apply_symlink_path`, `get_platform_custom_path`)
