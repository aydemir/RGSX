# Kritik Akış: Başlatma ve Manager Delegasyonu

> Geliştirici notu: Bu doküman `__main__.py` + `tvui.py` + `manager_launcher.py` +
> `rgsx_manager.py` kod akışını doğrulayarak yazılmıştır. Satır referansları commit
> `c5c5685` (Faz 9) itibarıyla geçerlidir.

## Özet

RGSX iki process üzerinde çalışır: **TV UI** (pygame) ve **Download Manager** daemon'ı
(`rgsx_manager.py`). İndirmeler her zaman manager'a delege edilir; manager yoksa TV UI
yerel (fallback) kuyruğa düşer. Web sunucusunu da manager çalıştırır
(`rgsx_web.run_server`), TV UI kendisi web sunucusu başlatmaz.

**Faz 6-5 sonrası yapı:** `__main__.py` yalnızca giriş noktasıdır (DPI + logging bootstrap
+ `from tvui import main`). TVUI boot/ana döngü `tvui.py`'de; manager spawn/supervisor
`manager_launcher.py`'de (`ensure_manager` + watchdog tabanlı `_start_manager_supervisor`).

```
RetroBat .bat
   └── __main__.py ── asyncio.run(tvui.main())
        └── tvui.py ── ensure_manager() (manager_launcher.py) ──┐
                              │  sağlık kontrolü: /api/health
                              ├── sağlıklı? ──True──► manager_available=True (delegasyon)
                              └── False ──► subprocess.Popen(rgsx_manager.py --port --minimized)
                                            └── poll: 30 sn, her turda gerçek portu oku
                                     sonra (manager varsa):
                                            ├── _start_manager_supervisor()  (watchdog respawn)
                                            └── _start_manager_sse_listener()
                                                  └── _manager_sse_worker() (daemon thread)
```

## 1. Giriş noktaları

| Mod | Dosya | Girdi |
|---|---|---|
| TVUI lansmanı | `__main__.py` | `roms\windows\RGSX Retrobat.bat` |
| TVUI boot + ana döngü | `tvui.py:585` (`main()`) | `__main__.py` dispatch |
| Manager daemon | `rgsx_manager.py:938` (`main()`) | `pythonw.exe rgsx_manager.py --minimized` (auto-start) |
| Manager spawn / supervisor | `manager_launcher.py:170` (`ensure_manager`), `:220` (`_start_manager_supervisor`) | `subprocess.Popen([...], --port, --minimized)` |
| Web UI + SSE | `rgsx_manager.py` | `rgsx_web.run_server(handler_class=ManagerHandler)` |

## 2. `ensure_manager()` (manager_launcher.py:170)

Adım adım davranış:

1. **Port okuma:** `rgsx_settings.get_manager_port()` → `config.manager_port`
   (exception'da varsayılan 5000).
2. **Local mod kontrolü:** `--ui-only` argümanı veya `RGSX_NO_MANAGER=1` → manager
   başlatmaz, `config.manager_available=False`, `False` döner. TV UI kendi kuyruğunu yönetir.
3. **Sağlık kontrolü:** `_manager_healthy(port)` → `GET /api/health` (timeout 2s), yanıt
   `success && manager` ise `manager_available=True` → yeni process **spawn edilmez**.
4. **Spawn:** `_spawn_manager_process(port)`; manager script yoksa local moda düşer.
   Windows'ta `CREATE_NO_WINDOW` ile arka planda başlatır; log `rgsx_manager_spawn.log`'a yazılır.
5. **Poll döngüsü (30 sn):** `_wait_for_manager_ready(proc, timeout=30)` — her 0.5 sn'de:
   - **Faz 4 kritik:** `get_manager_port()` her turda yeniden okunur — manager istenen port
     doluysa `5000+N`'ye geçip settings'e yazdığı için gerçek port poll ile yakalanır.
   - `_manager_healthy` True → `manager_available=True`, `True` döner.
   - Spawn edilen process erken çıktıysa (`proc.poll()` None değil) → local mod.
6. 30 sn dolarsa → local mod, `False`.

`tvui.main()` `ensure_manager()`'dan sonra (yalnızca `manager_available` True ise)
`_start_manager_supervisor()` çağırır (tvui.py:614-615).

## 3. Çift manager koruması (kritik mimari)

- **TVUI tarafı:** `ensure_manager()` sağlıklı manager görürse spawn etmez.
- **Son savunma (`rgsx_manager.main()`):** `main()` içinde `manager_healthy()` True ise
  "already running" deyip tray ikonu oluşturmadan `return 0` ile çıkar.
- Bu iki katman, iki manager'ın aynı portta yarışmasını engeller.

## 4. Manager `main()` akışı (rgsx_manager.py:938)

1. CLI argümanı yoksa `get_manager_port()` / `get_manager_host()`.
2. `--auto-start-install/remove` → registry + `_set_autostart_pref` + çık.
3. `manager_healthy` → tekrar çalışan var, çık.
4. **Faz 4:** `_find_available_port(preferred, host, 100 deneme)` — istenen port doluysa
   `preferred+1..+100` dener; hepsi doluysa `0` (net hata). Farklı port seçilirse
   `set_manager_port()` ile kalıcılaştırır. **Başka process asla öldürülmez**
   (`run_server(kill_conflicts=False)`).
5. **Auto-start:** Windows + tercih True + kurulu değil → registry'ye kur.
6. Thread'ler: `download_queue_worker` (network/queue.py — kuyruk işçisi) + `_broadcaster_loop`
   (SSE) + `set_state_emitter(_broadcast)` (Faz 8 durum SSE'si).
7. `_resume_interrupted_downloads()` — history'de "Téléchargement"/"Downloading"/"Paused"
   olan girdileri kuyruğa geri ekler (torrent kısmi veriden, HTTP baştan).
8. Tray ikonu (yoksa) → `run_server(handler_class=ManagerHandler)`.

## 5. SSE yansıması (TV UI ← Manager)

`_start_manager_sse_listener()` (tvui.py:410) daemon thread başlatır:

```
_manager_sse_worker (tvui.py:450) ── urllib GET http://127.0.0.1:{port}/api/events (timeout 60s)
      └── _stream_sse: 4KB chunk oku, SSE satır ayrıştır (event:/data:)
            └── _apply_manager_event(event_type, payload, last_seen) (tvui.py:493)
```

`_apply_manager_event` event → config eşlemesi:

| Event | config hedefi |
|---|---|
| `snapshot` / `history` | `config.history` + `_detect_download_completions` (toast) |
| `snapshot` / `progress` | `config.download_progress` |
| `snapshot` / `queue` | `config.download_queue` + `config.download_active` |
| `snapshot` / `downloaded` | `config.downloaded_games` |

Her güncellemede `config.needs_redraw = True` → pygame döngüsü yeniden çizer. Bağlantı
koparsa worker 3 sn bekleyip yeniden bağlanır; `manager_available=False` ise 2 sn bekle
(manager_available bayrağı asla poll ile geri açılmaz — ayrı akış, başlatma sırasında set edilir).

`_detect_download_completions` (tvui.py:532): `last_seen` dict'i ile önceki status
karşılaştırır; `Downloading→Download_OK/Completed` = başarı toast'ı, `→Erreur/Error` = hata toast'ı.

## 6. Yerel resume (TVUI, manager'sız mod)

`_resume_tvui_downloads()` (tvui.py:414) — sadece `--ui-only`/fallback'te çalışır;
manager aktifken TVUI tarafı atlanır (tvui.py:626-629, çift kuyruğu önler). Torrent resume
girdisi varsa `_prewarm_qbittorrent_startup()` ile qBittorrent önceden başlatılır; her girdi
`start_or_queue_download` ile yerel kuyruğa konur.

## 7. Port özeti

| Servis | Port | Kaynak |
|---|---|---|
| Manager HTTP + SSE + WebUI | 5000 (doluysa 5000+N) | `rgsx_settings.json` `manager_port` |
| qBittorrent WebUI | 18572 (Faz 3: doluysa 18572+N) | `admin`/settings şifresi |

## İlgili dosyalar

- `__main__.py` (yalnız giriş noktası — DPI + logging + `tvui.main`)
- `tvui.py` (boot, `main()`, SSE client, yerel resume)
- `manager_launcher.py` (`ensure_manager`, `_spawn_manager_process`, `_wait_for_manager_ready`,
  `_start_manager_supervisor`)
- `rgsx_manager.py` (daemon, tray, SSE broadcaster, port yönetimi)
- `rgsx_web/` (`run_server`, `RGSXHandler`, `ManagerHandler` temeli)
- `rgsx_settings.py` (`get/set_manager_port`, `get/set_manager_host`)
