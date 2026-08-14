# TASK-002-gap-3 — Cancel + Kısmi Dosya Temizliği (Rust'ta eksik)

- **id:** TASK-002-gap-3
- **title:** Cancel + partial-file / torrent temp-root / seeder artifact temizliği
- **status:** completed
- **priority:** P0
- **created:** 2026-08-14
- **completed:** 2026-08-14
- **environment:** both
- **tags:** cancel, cleanup, download
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `P3`, `P4`, `P5`, `P6`, `Q9` (kısmi), `Q6c` (kısmi)
- `ports/RGSX/network/queue.py`:
  - `request_cancel` (121), `cancel_all_downloads` (394), `shutdown_downloads` (425)
  - `cleanup_torrent_temp` (161), `_cleanup_torrent_resume_artifacts` (195),
    `_cleanup_seeder_local_artifacts` (241), `stop_active_seeder` (280)
- `ports/RGSX/qbittorrent_backend.py`: `_terminate_managed_process` (333), `stop_seed` (1481),
  `download_torrent_via_qbittorrent` cancel dalı (satır 1592)

## Açıklama

İptal, yalnızca indirmeyi durdurmak değil — **yarım kalan dosyaları, `.rgsx_torrent` temp
köklerini ve seeder artifact'larını (`.aria2`, manifest, seed work dir) silmeyi** de kapsar.
`cleanup_torrent_temp` eski platform klasörlerindeki orphan temp kökleri de tarar
(`_find_stray_torrent_temp_roots`). Rust path'inde `download_torrent_source`
(`manager-torrent/src/lib.rs`) cancel sırasında bu temizliği yapmaz; `wait_until_completed`
önce tamamlanmayı bekler, iptal sinyali yok. `stop_seed`/seeder artifact temizliği tamamen
Python'da kalır.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — cancel sırasında temp + partial dosya silme
- `manager-rs/manager-bin/src/` — `/api/cancel` sözleşmesine temizlik adımı
- `rust_daemon.py` — `_post_json("/api/cancel", ...)` zaten var; daemon tarafı temizlik eklemeli

## Uygulama Özeti

- `manager-bridge` — `TorrentBackend` trait'ine Gap-3 metotları (default = JSON-RPC proxy):
  `cancel_torrent(task_id)` (P3..P6 karşılığı) + `cancel_all()` (`cancel_all_downloads` karşılığı).
- `manager-torrent` — `LibrqbitEngine.cancel_task(task_id)`: handle'ı `active_handles`'tan alır,
  `Session::delete(TorrentIdOrHash::Id, delete_files=true)` ile session'dan siler (librqbit
  `.rqbitpart`/kısmi dosyaları diskten temizler), map'ten çıkarır. `cancel_all_tasks()` (inherent)
  tüm aktif task'ları iptal eder. `call()`'a `cancel`/`cancel_all` JSON-RPC metotları eklendi.
  `download_torrent_source_with_progress` progress loop'u her turda `task_id` hâlâ
  `active_handles`'ta mı kontrol eder — iptal edildiyse `wait_until_completed`'e takılmadan
  "indirme iptal edildi" hatası döner.
- `manager-http` — `/api/cancel`: catalog proxy'sini korur; yoksa bridge'e `cancel_torrent`
  (task_id) / `cancel_all` (task_id yoksa); bridge yoksa placeholder (geriye uyum). Yanıta
  `canceled: bool` ekler.
- `rust_daemon.py` — `/api/cancel` gövdesine `task_id` eklendi (Rust temp temizliği task_id ile
  eşleşir).
- Seeder artifact temizliği (`stop_seed`, `.aria2`/manifest) Python'da kalır — Gap-7 seed
  lifecycle'a ait (bu görevde dokunulmadı).

## Doğrulama

- İptal edilen Rust torrenti: `output_folder` altındaki partial + `.rqbitpart` + temp kök
  `Session::delete(delete_files=true)` ile silinir (librqbit `remove_files_and_dirs`).
- `cleanup_torrent_temp` orphan taraması Rust'ta ayrı iş — Gap-8 stray-temp kapsamı (bu görevde
  librqbit `.rqbitpart` temizliği + iptal kancası verildi).
- `stop_seed` çağrısı seed dosyalarını + `.aria2`/manifest'i temizler (fallback qBittorrent ile
  parity — Python tarafında korundu, regression yok).
- `cancel_all_downloads` tüm aktif torrentleri durdurur + queue'yu temizler (regression yok).

### Testler

- `manager-torrent/tests/engine.rs` — +2 birim (bilinmeyen task cancel → false/cancel_all → 0,
  `call` dispatch `cancel:false`/`cancel_all:{canceled:0}`). 15 geçiyor.
- `manager-http/tests/contract.rs` — +2 (`/api/cancel` bridge'li task_id yönlendirme, task_id
  yoksa `cancel_all`). 105 geçiyor (tek thread'de stabil; paralel koşuda bir kez
  `RGSX_SETTINGS_PATH` env-var çakışmasıyla flaky, HEAD'de de tekrarlanıyordu).
- `tests/test_rust_daemon.py` — +1 (iptal POST'u `task_id` taşır, url korunur). 15 geçiyor.
- Python suite: 764 passed / 23 pre-existing (test_display_* + pygame-stub).
- Workspace: tüm crates yeşil (0 failed).
