# TASK-002-gap-7 — Seed Lifecycle + Password Migration (Rust'ta kısmen eksik)

- **id:** TASK-002-gap-7
- **title:** Seed lifecycle (promote-to-seed, _seed_status_worker, has_active_seed, stop_seed) + qBittorrent password migration
- **status:** closed (out-of-scope)
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

---

## Parite Denetimi 2026-08-15 — Ek Maddeler

### Madde A: Seed yaşam döngüsü hâlâ YOK (❌ / ⚠️ KISMİ — parity denetimi teyidi)

- Python: `qbittorrent_backend.py:1684` `_promote_active_download_to_seed`, `:1450`
  `_seed_status_worker`, `:1440` `has_active_seed`, `:1481` `stop_seed`.
- Rust: `manager-torrent/src/lib.rs:129-208` indirip `link_or_copy` eder; **seed takibi / status
  worker / `stop_seed` yaşam döngüsü YOK**.
- Bu TASK'ın asıl kapsamı zaten buydu; parity denetimiyle teyit edildi (dosya:satır kanıtlandı).
- Bağımlılık: `TASK-002-gap-16` (dosya seçimi) ve `TASK-002-gap-8` (izolasyon) ile
  `manager-torrent/src/lib.rs` paylaşımı — sıralı ele alınmalı.

## 2026-08-18 KAPATMA — kapsam dışı (kullanıcı kararı, gap-18 ile tutarlı)

Kullanıcı: gap-7 (seed yaşam döngüsü + qBittorrent şifre migration) Rust portu kapsamı dışı.
Gerekçe: Bu özellikler **qBittorrent/Python fallback path'ine** aittir (`qbittorrent_backend.py`
`_promote_active_download_to_seed`/`_seed_status_worker`/`has_active_seed`/`stop_seed` ve Faz 5
şifre migration). Rust librqbit embedded modelinde seed takibi kasıtlı yoktur (indir → link/copy → biter;
`manager-torrent/src/lib.rs:195-196` yorumu teyit eder). gap-18 (qBittorrent UI) out-of-scope kararıyla
tutarlı olarak kapatıldı.

Not: "Aktif indirmeyi durdur" ihtiyacı Rust'ta ZATEN karşılanıyor — `/api/cancel` ucu
(`lib.rs:58`) + `cancelDownload()` UI butonu (`App.vue:452`) + CANCELED durumu. Eksik olan yalnızca
"bitmiş ama seeding devam eden torrent'i durdur+sil" (`stop_seed`); bu qBittorrent kavramı olduğundan
Rust portuna port EDİLMEYECEK. İstenirse librqbit path'i için dar bir "stop & delete seeding" özelliği
ayrı görev olarak açılabilir.
