# Kritik Akış: İndirme Pipeline'ı (HTTP Resume + Torrent + Kuyruk)

> Geliştirici notu: Satır referansları commit `7f0199f` itibarıyla geçerlidir.
> Modül: `network.py` (5667 satır), `controls.py`, `rgsx_manager.py`.

## Özet

İndirme isteği hangi istemciden gelirse gelsin (TVUI, WebUI, CLI, tray) **tek karar
noktasına** düşer. Manager aktifse HTTP ile delege edilir, değilse yerel kuyruk/thread
kullanılır. İki indirme türü vardır: **HTTP direkt** (1Fichier/AllDebrid/DebridLink/
RealDebrid/TorBox) ve **torrent** (gömülü qBittorrent).

```
TVUI controls.py ── start_or_queue_download ──┐
WebUI /api/download ── manager ───────────────┼──► download_queue_worker
CLI rgsx_cli.py ──────────────────────────────┤          │
                                              │    slot boşsa: thread ── asyncio.run(download_rom)
                                              │      ├── HTTP: _stream_response_to_path (.part + Range)
                                              │      └── torrent: qBittorrent API
```

## 1. İstek karar noktası: `start_or_queue_download` (controls.py:848)

1. `config.manager_available` True → `_delegate_download_to_manager()`:
   - background thread'de `POST http://127.0.0.1:{port}/api/download`
   - body: `{platform, game_name, url, mode: "now"}`
   - sonuç `("queued", "manager")` — TVUI slot saymaz, manager yönetir.
2. Manager yoksa yerel: aktif slot (`active_download_count`) ≥ `max_simultaneous_downloads`
   ise `_queue_download()` → `config.download_queue`'ya ekle, `("queued", task_id)`.
3. Slot boşsa doğrudan başlat:
   - `is_1fichier_url(url)` → `download_from_1fichier` (API anahtarı yoksa **gratuit** mod)
   - değilse → `download_rom`
   - `asyncio.create_task` + `_register_download_task(...)` (slot+1, task kaydı).

## 2. Kuyruk işçisi: `download_queue_worker` (network.py:1622)

Manager `main()` tarafından daemon thread olarak başlatılır (`rgsx_manager.py:896`).

```python
while True:
    active < max_dl and config.download_queue:   # slot kontrolü
        job = config.download_queue.pop(0)
        active += 1; config.download_active = True
        # is_1fichier? → asyncio.run(download_from_1fichier(...)) : download_rom(...)
        threading.Thread(target=..., daemon=True).start()
    time.sleep(1)
```

- Slot serbest kaldığında `notify_download_finished()` (network.py:1650) çağrılır:
  `active_download_count -= 1`.
- Worker hata durumunda 2 sn bekleyip devam eder — kuyruk asla ölmez.

## 3. HTTP indirme + resume (network.py:884–1011)

Resume mekanizması dört yardımcı fonksiyondan oluşur:

| Fonksiyon | Rol | Konum |
|---|---|---|
| `_http_part_path(dest)` | `.part` dosya yolu: `f"{dest}.part"` | 884 |
| `_http_resume_offset(dest)` | Mevcut `.part` boyutu (byte), yoksa 0 | 889 |
| `_http_parse_content_range(h)` | `bytes a-b/total` → total boyut | 901 |
| `_stream_response_to_path(...)` | Akışı yazar, resume/progress yönetir | 914 |

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

## 4. `download_rom` (network.py:3115) — torrenthandler + history + dispatcher

1. `parse_torrent_download_url(url)` → `torrent_meta` (yoksa None) — `rgsx+torrent://` için
   `utils.is_torrent_download_url` zorlama yolu.
2. **Duplikasyon koruması** (`urls_in_progress` + `urls_lock`): URL zaten iniyorsa
   `url_done_events[url]` Event'ine 30 dk bekler, `url_results[url]` önbelleğinden sonucu döner.
3. `progress_queues[task_id]` + `cancel_events[task_id]` oluştur.
4. `download_thread()` (iç fonksiyon):
   - `config.history = load_history()` → mevcut girişi `"Downloading"`'e resetle ya da yeni
     giriş ekle; `_save_history_with_feedback` ile diske yaz.
   - Platform özel klasör (`get_platform_custom_path`) yoksa `config.platform_dicts`'ten
     `folder` çözülür; symlink ayarı uygulanır.
   - `torrent_meta` varsa torrent akışına (qBittorrent), yoksa HTTP akışına gider.
   - Bitişte history güncellenir (`Download_OK`/`Erreur`), `notify_download_finished()` çağrılır,
     `url_results`/`url_done_events` temizlenir.

## 5. 1Fichier: `download_from_1fichier` (network.py:4268)

- API anahtarları varsa provider akışı (AllDebrid/DebridLink/RealDebrid/TorBox), yoksa
  **gratuit** mod: HTTP istekler + bekletme (wait) mantığı.
- `is_1fichier_url` (network.py:5665) URL tipini belirler; `start_or_queue_download` ve
  `download_queue_worker` bu ayırımı yapar.

## 6. Torrent (gömülü qBittorrent)

- qBittorrent-nox (embedded) `qbittorrent_backend.py` API ile yönetilir; WebUI 18572.
- Kısmi veri qBittorrent'te korunur → **restart sonrası `_resume_interrupted_downloads`
  (rgsx_manager.py:716)** history'den "Downloading/Paused" girdilerini kuyruğa geri ekler,
  torrent kaldığı yerden devam eder.
- TVUI yerel resume `_prewarm_qbittorrent_startup()` ile torrent girdisi varsa qB'yi önceden başlatır.

## 7. İptal / Pause / Resume

| Eylem | Mekanizma |
|---|---|
| Cancel | `cancel_events[task_id].set()` → akış iptal, `.part` sil; manager: `/api/cancel` |
| Pause | `pause_events[task_id].set()` → chunk döngüsü pause; `/api/pause` (toplu) |
| Resume | `pause_events[task_id].clear()` → kaldığından devam; `/api/resume` |
| Slot | `active_download_count`; bittiğinde `notify_download_finished()` |

## İlgili dosyalar

- `network.py` (akış, resume, queue worker, cancel/pause)
- `controls.py` (`start_or_queue_download`, `_delegate_download_to_manager`)
- `rgsx_manager.py` (worker spawn, `_resume_interrupted_downloads`, `/api/*`)
- `qbittorrent_backend.py` (torrent API)
- `rgsx_settings.py` (`apply_symlink_path`, `get_platform_custom_path`)
