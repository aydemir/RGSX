# Eşzamanlılık ve Süreç Modeli Referansı

> Faz 4-6 mimarisi: tek süreçte 5+ thread + isteğe bağlı manager daemon süreci.
> Satır referansları commit `c5c5685` (Faz 9) itibarıyla geçerlidir.

## Süreç/thread haritası

| Aktör | Süreç | Rol |
|---|---|---|
| TV UI ana döngü | TV (veya manager daemon ile aynı süreç) | pygame olay döngüsü + `process_key_repeats` |
| `_manager_sse_worker` (tvui.py:450) | TV | Manager'ın SSE'sini dinler, `_apply_manager_event` ile uygular |
| `download_queue_worker` (queue.py:91) | manager (veya TV fallback) | Tek tüketici: kuyruktan sırayla indirir |
| indirme thread'leri (network) | TV fallback | `download_threads[task_id]`, URL başına bir thread |
| `_history_writer_loop` (history.py:234) | TV | Async history.json yazıcı |
| `_manager_supervisor_loop` (manager_launcher.py:226) | TV | 5 sn'de bir health poll, respawn kararı |
| `_broadcaster_loop` (rgsx_manager.py) | manager | SSE snapshot/progress yayını |

## Merkezi kilit (`thread_safety.py`)

- **Master kilit**: `_network_lock = threading.RLock()` — tek kilit, deadlock'u basit tutar
  (ince taneli kilitleme faydasız; tek kilide güvenilir).
  - `network_lock()` context manager (thread_safety.py:29)
  - `with_network_lock` dekoratörü (thread_safety.py:45)
- **Per-sözlük kilitleri** (nadiren gerekli, ileri düzey): `pause_events_lock`,
  `download_threads_lock`, `cancel_events_lock`, `progress_queues_lock`,
  `torrent_temp_roots_lock`, `url_done_events_lock`, `url_results_lock` +
  config tarafı: `download_tasks_lock`, `download_progress_lock`, `download_queue_lock`,
  `history_lock`.
- **Kolaylık fonksiyonları** (thread_safe sarmalar):
  - `get_pause_event(task_id)` / `set_pause_event` / `clear_pause_event`
  - `register_download_thread` / `unregister_download_thread`
  - `get_cancel_event` / `request_cancel_task(task_id)` / `register_cancel_event`

Kural: network/config paylaşımlı durumuna dokunan yeni kod `network_lock()` (veya
`with_network_lock`) kullanmalıdır; module state network/__init__.py:17-31'de tanımlıdır
(`progress_queues`, `cancel_events`, `pause_events`, `download_threads`,
`torrent_temp_roots`, `_app_shutting_down`, `urls_in_progress`, `urls_lock`,
`url_results`, `url_done_events`).

## Tek tüketici kuralı (indirme)

- `config.queue_worker_running=True` (manager modu) → **tek tüketici**
  `download_queue_worker` (queue.py:91); HTTP `POST /api/download` bu kuyruğa yazar.
- `False` (standalone TV / fallback) → legacy zincir `_process_queued_download`
  (handlers_download.py:19) + `controls/downloads._launch_next_queued_download`.
- İki tüketici asla aynı anda çalışmaz; `queue_worker_running` bayrağı kimin yönettiğini belirler.

## Watchdog karar mantığı (`watchdog.py`) — saf, bağımlılıksız

Manager durum makinesi: `INIT → RUNNING ⇄ DEGRADED → UNRESPONSIVE → RESTARTING → CRASHED`.

- `HysteresisMonitor(degrade_threshold=3, unresponsive_threshold=6)` (watchdog.py:29)
  - ardışık başarısızlık ≥ degrade → `DEGRADED`
  - ardışık başarısızlık ≥ unresponsive → `UNRESPONSIVE`
  - **herhangi bir başarı** sayaçları sıfırlar → `RUNNING` (hysteresis: seyrek hata kalıcı
    durum üretmez)
  - `report(healthy) → state`; `reset()` sayaçları sıfırlar
- `RestartLimiter(max_restarts=3, window_seconds=3600)` (watchdog.py:66)
  - kayan pencerede max restart; `record_restart()` limit doluysa `False` (artış yok);
    limit dolunca çağıran `CRASHED`'e geçer

## Manager supervisor (manager_launcher.py)

Sabitler: `_SUPERVISOR_POLL_SECONDS=5.0`, `DEGRADE_THRESHOLD=3`, `UNRESPONSIVE_THRESHOLD=6`,
`MAX_RESTARTS=3`, `RESTART_WINDOW_SECONDS=3600`.

- `ensure_manager()` (:170) — manager'ı başlatır (varsa tekrar kullanmaz).
- `_start_manager_supervisor()` (:220) — daemon thread `_manager_supervisor_loop`.
- `_manager_supervisor_loop()` (:226) — 5 sn'de bir `/api/health` poll; `HysteresisMonitor` +
  `RestartLimiter`; `UNRESPONSIVE` + limit izni → `_spawn_manager_process(port)` respawn,
  ardından `_wait_for_manager_ready(timeout=30)`.
- `stop_web_server()` (:260) — legacy uyumluluk stub'ı (web sunucusu artık manager'da).

## SSE senkronizasyonu (TVUI ← manager)

- `_manager_sse_worker` (tvui.py:450) — `/api/events` SSE akışını okur.
- `_apply_manager_event` (tvui.py:493) — olayları TV UI durumuna uygular
  (progress/history/queue/downloaded + Faz 8 `download_state`).
- `_detect_download_completions` (tvui.py:532) — manager yokken/SSE kaçırdığında
  tamamlanma tespiti (fallback).
- `_resume_tvui_downloads` (:414) — TV başlarken bekleyen indirmeleri sürdürür.

## Kilit hiyerarşisi notu

- Master `_network_lock` RLock olduğu için iç içe alım güvenlidir (aynı thread).
- Kilidin `history_lock` ayrıdır: `history.py` async writer'ı kendi kuyruğunu kullanır;
  UI okumaları kilit altında, yazımlar `_async_write_json` (history.py:330) üzerinden.
- `_app_shutting_down` bayrağı kapatma akışında yeni download başlatmayı engeller (Faz 8).

## İlgili dosyalar

- `thread_safety.py` — merkezi kilitler
- `watchdog.py` — hysteresis + restart limiti
- `manager_launcher.py` — supervisor loop + respawn
- `rgsx_manager.py` — manager süreci, SSE yayını, `/api/health`
- `tvui.py` — SSE dinleyici + download tespiti
- `history.py` — async writer
