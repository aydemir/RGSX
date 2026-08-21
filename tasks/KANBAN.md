# RGSX Parite Denetimi — Roadmap & Kanban (2026-08-15)

Python (`ports/RGSX/`) ↔ Rust (`RGSX/manager-rs/`) tam parite taraması sonucu çıkan ❌/⚠️
maddelerinin kalıcı görev birimlerine dönüştürülmüş halidir. Retry/backoff (TASK-002-gap-1)
kapsam dışıdır.

Bu dosya git-based markdown kanban kartıdır. Her görev tek satırlık karttır; detay için
`tasks/gap/<dosya>` referans verilir.

---

## ÖNCELİK SIRALI ROADMAP

| Sıra | TASK dosyası | Özet | Neden bu öncelik | Bağımlılık |
|---|---|---|---|---|
| 1 | `TASK-002-gap-13-support-redaction.md` | /api/support gizli redaksiyon (saf-Rust mod) | 🔴 P0 güvenlik — secret sızıntısı | yok |
| 2 | `TASK-002-gap-10-history-sse.md` | History disk kalıcılığı + clear_history aktif koruma + downloaded_games + timestamp + SSE throttle | 🟡 P1 veri kaybı (geçmiş/indirme) | gap-1 (finalize) |
| 3 | `TASK-002-gap-5-disk-space.md` | Disk alanı ön-kontrolü (InsufficientDiskSpace yükseltilmiyor) + permission pre-check | 🟡 P1 sessiz hata yutma / veri bozulması | yok |
| 4 | `TASK-002-gap-12-download-orchestration.md` | Concurrency limiti + slot + dedup + FIFO + global pause (HTTP-direct) | 🟡 P1 kaynak tükenmesi / davranış sapması | gap-1 (api.rs spawn) |
| 5 | `TASK-002-gap-16-torrent-selection.md` | Torrent çok dosya seçimi + öncelik | 🟡 P1 yanlış/gereksiz veri indirme | gap-7/8 ile lib.rs paylaşımı |
| 6 | `TASK-002-gap-7-seed-lifecycle.md` | Seed yaşam döngüsü (status worker/stop_seed) | 🟢 P2 kullanıcı "paylaşımı durdur" bekler | gap-16/8 ile lib.rs |
| 7 | `TASK-002-gap-6-extract.md` | auto_extract persist + bozuk arşiv bütünlük testi | 🟢 P2 arşiv açma davranışı | yok |
| 8 | `TASK-002-gap-8-stray-temp.md` | Stray temp temizliği + disk yazma izolasyonu/kaynak koruma | 🟢 P2 birikim / izolasyon | gap-16 ile lib.rs |
| 9 | `TASK-002-gap-14-game-filters.md` | game_filters saf mantık + Rust testi | 🟢 P2 test kapsamı açığı | TASK-006 (BELİRSİZ) |
| 10 | `TASK-002-gap-15-rust-endpoint-tests.md` | Rust-only uç nokta testleri (languages/scan/es-input) + dil auto-detect | 🟢 P2 test kapsamı açığı | yok |
| 11 | `TASK-002-gap-17-settings-schema.md` | Eksik ayar alanları (background_theme, web_service_at_boot, gamelist days, app_version, path model) | ⚪ P3 nice-to-have / round-trip veri kaybı | TASK-006 (BELİRSİZ) |

---

## KANBAN KARTLARI (tek satır)

### P0 — Güvenlik
- [x] `TASK-002-gap-13` Support ZIP redaction (saf-Rust) — `tasks/gap/TASK-002-gap-13-support-redaction.md` (✅ 2026-08-18: `manager-core/src/secrets.rs` `redact_secrets` + `api.rs::build_support_zip` + `test_support_redacted_offline`)

### P1 — Kritik / Veri Kaybı
- [x] `TASK-002-gap-10` History disk persist + clear_history aktif koruma + downloaded_games + SSE throttle — `tasks/gap/TASK-002-gap-10-history-sse.md` (2026-08-18 kapatıldı: A/B/C/D/E kodda mevcut)
- [x] `TASK-002-gap-5` Disk alanı ön-kontrolü + permission pre-check — `tasks/gap/TASK-002-gap-5-disk-space.md` (✅ 2026-08-18: `manager-core/src/disk.rs` + `DownloadError::InsufficientDiskSpace` `http/mod.rs:245`)
- [ ] `TASK-002-gap-12` Download orchestration (concurrency/slot/dedup/FIFO/global pause) — `tasks/gap/TASK-002-gap-12-download-orchestration.md` (⚠️ 2026-08-18: semaphore+dedup+global pause mevcut; FIFO sıralama belirsiz)
- [x] `TASK-002-gap-16` Torrent dosya seçimi + öncelik — `tasks/gap/TASK-002-gap-16-torrent-selection.md` (✅ 2026-08-18: `manager-torrent/src/lib.rs` `only_files`/`only_files_regex`)

### P2 — Orta
- [ ] `TASK-002-gap-7` Seed lifecycle (status worker/stop_seed) — `tasks/gap/TASK-002-gap-7-seed-lifecycle.md` (⚠️ 2026-08-18: pause_all/resume_all var; stop_seed bulunamadı)
- [x] `TASK-002-gap-6` auto_extract persist + bozuk arşiv bütünlük — `tasks/gap/TASK-002-gap-6-extract.md` (✅ 2026-08-18: `extract.rs::should_force_extract` truth-table + test; `auto_extract` persist)
- [ ] `TASK-002-gap-8` Stray temp + disk yazma izolasyonu — `tasks/gap/TASK-002-gap-8-stray-temp.md` (⚠️ 2026-08-18: `remove_dir_all` temizlik var; startup stray-temp reaper belirsiz)
- [x] `TASK-002-gap-14` game_filters saf mantık + test — `tasks/gap/TASK-002-gap-14-game-filters.md` (✅ 2026-08-18: `api.rs:962` persist → `Settings.extra` + round-trip test)
- [ ] `TASK-002-gap-15` Rust-only endpoint testleri + dil auto-detect — `tasks/gap/TASK-002-gap-15-rust-endpoint-tests.md` (⚠️ 2026-08-18: Rust endpoint testleri mevcut; `detect_lang`/`accept_language` = YOK)

### P3 — Nice-to-have
- [ ] `TASK-002-gap-17` Settings şema parity (eksik ayar alanları) — `tasks/gap/TASK-002-gap-17-settings-schema.md` (⚠️ 2026-08-18: `web_service_at_boot`/`auto_extract` var; `background_theme`/`app_version`/`gamelist_days` = YOK)

---

## DİĞER TAMAMLANAN GÖREVLER

- [x] `TASK-011` Native `load_games` Python list-of-lists games json formatını parse edemiyor — `tasks/done/TASK-011-games-parser.md` (✅ 2026-08-21: `catalog.rs:393-422` her iki formatı çözüyor + `games_list_format` contract testi; görev yanlışlıkla silinmişti, `done/`'a taşındı)

---

## BELİRSİZ MADDELER (kullanıcı onayı gerekir)

- gap-12: concurrency gate mekanizması (Semaphore mı / slot sayacı mı) ve FIFO worker şekli — BELİRSİZ
- gap-13: redaksiyon port yaklaşımı (security.py port mu / merkezi redact_secrets mı) — BELİRSİZ
- gap-14: filtre mantığı hangi crate'e (core mı / scan mı) — BELİRSİZ; TASK-006 ile çakışma kontrolü
- gap-16: hedef dosya seçim kriteri (en büyük / is_zip_non_supported / game_name) — BELİRSİZ
- gap-17: `RGSX_APP_DIR`/`RGSX_CONFIG_DIR` path modeli bilinçli fark mı yoksa açık mı — BELİRSİZ
