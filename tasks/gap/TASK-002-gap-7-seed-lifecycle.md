# TASK-002-gap-7 — Seed Lifecycle + Password Migration (Rust'ta kısmen eksik)

- **id:** TASK-002-gap-7
- **title:** Seed lifecycle (promote-to-seed, _seed_status_worker, has_active_seed, stop_seed) + qBittorrent password migration
- **status:** todo
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** seed, qbittorrent, password
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `Q6ok → Q7 → Q8`, `Q5` (password migration kısmı)
- `ports/RGSX/qbittorrent_backend.py`:
  - `_promote_active_download_to_seed` (1085), `_seed_status_worker` (1450),
    `has_active_seed` (1440), `stop_seed` (1481)
  - `maybe_migrate_qbittorrent_password` (1331), `ensure_qbittorrent_password_secured` (1229),
    `regenerate_qbittorrent_password` (1261), `change_webui_password` (1379)

## Açıklama

Python qBittorrent path'inde indirme bitince torrent **seed** olarak tutulur; `_seed_status_worker`
periyodik olarak seeding peers/ul_speed'ı history'ye yazar, kullanıcı `stop_seed` ile torrent+file
siler. Rust `LibrqbitEngine` **seed takibi/status worker'ı yapmaz** — sadece indirip link/copy eder.
Ayrıca qBittorrent WebUI şifre migration'ı (Faz 5) tamamen Python'da kalır; librqbit embedded
modda şifre kavramı yoktur ama fallback qBittorrent path'i için korunmalıdır.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — seed tracking + status worker (gerekirse)
- `manager-rs/manager-bin/src/` — `/api/seed/stop` ucu
- `rust_daemon.py` — seed durumu yansıtma (`_mirror_progress` genişletme)

## Doğrulama

- İndirme bitince seed durumu history'ye yansır (peers/ul_speed).
- `stop_seed(task_id/url)` torrent+file'ı temizler, başka ref yoksa.
- Şifre migration: mevcut qBittorrent kurulumunda regression yok (fallback path).
