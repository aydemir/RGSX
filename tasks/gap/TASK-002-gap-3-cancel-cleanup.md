# TASK-002-gap-3 — Cancel + Kısmi Dosya Temizliği (Rust'ta eksik)

- **id:** TASK-002-gap-3
- **title:** Cancel + partial-file / torrent temp-root / seeder artifact temizliği
- **status:** todo
- **priority:** P0
- **created:** 2026-08-14
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

## Doğrulama

- İptal edilen Rust torrenti: `output_folder` altındaki partial + `.rqbitpart` + temp kök silinir.
- `cleanup_torrent_temp` orphan taraması Rust'ta da yapılır (eski platform path'leri).
- `stop_seed` çağrısı seed dosyalarını + `.aria2`/manifest'i temizler (fallback qBittorrent ile parity).
- `cancel_all_downloads` tüm aktif torrentleri durdurur + queue'yu temizler (regression yok).
