# TASK-002-gap-29 — Global pause for HTTP-direct downloads

- **id:** TASK-002-gap-29
- **title:** Global pause/resume must also halt native HTTP-direct downloads (not only the torrent engine)
- **status:** done
- **priority:** P2
- **created:** 2026-08-18
- **updated:** 2026-08-18
- **environment:** both
- **tags:** download, pause, http-direct, orchestration
- **parent:** TASK-002

## Bağlam

`TASK-002-gap-12` download orchestration'ın çekirdek 4 maddesi tamamlandı; global pause'ın
yalnız torrent engine'i kapsaması, native HTTP-direct'i kapsamaması ayrı özellik olarak bu
göreve ayrıldı. **Artık native HTTP-direct indirmeleri de global pause'a uyuyor.**

## Python Referans Davranışı

- `ports/RGSX/network/queue.py:333-393` — `pause_all_downloads()` / `resume_all_downloads()` /
  `is_any_download_paused`. Global duraklatma aktifken yeni indirmeler başlamaz ve devam eden
  HTTP indirmeleri (stream) durdurulur; resume'da kaldığı yerden devam eder.

## Rust Uygulanan (2026-08-18)

- `manager-http/src/state.rs` — `StateData`'ya eklendi: `global_paused: bool`,
  `pause_resume: Arc<Notify>` (resume sinyali), `pause_signals: HashMap<String, Arc<Notify>>`
  (URL başına pause abort sinyali).
- `manager-http/src/api.rs` — `pause`/`resume` handler'larının native (bridge yok) dalı artık
  placeholder `queue_size` döndürmez: `pause` → `global_paused=true` + tüm `pause_signals`'ı
  `notify_one()` (devam eden indirmeleri abort eder); `resume` → `global_paused=false` +
  `pause_resume.notify_waiters()` (bekleyen döngüleri uyandırır).
- `native_ddl_download` döngüsü:
  - Loop başı: `global_paused` ise `pause_resume` sinyaline kadar bekler (yeni indirme başlamaz).
  - `download_async` bir `tokio::select!` içine alındı; `pause_sig.notified()` dalı `CancelFlag`'i
    set ederek devam eden indirmeyi abort eder (`paused_now` bayrağı ile retry akışına girer,
    loop başı `global_paused` kontrolü resume'a kadar bekletir). `cancel`/`shutdown` da aynı
    `CancelFlag` üzerinden keser.
  - Completion'da `pause_signals` temizlenir (retry_in_flight/cancel_signals ile birlikte).
- Range resume zaten mevcut (`HttpDownloader`) → resume'da indirme kaldığı yerden devam eder.

## Doğrulama

- ✅ Global pause aktifken devam eden native HTTP-direct indirme `CancelFlag` ile durur.
- ✅ Pause altında yeni HTTP-direct indirme başlamaz (loop-top bekleme).
- ✅ Resume'da indirme `pause_resume.notify_waiters()` ile devam eder (Range resume).
- ✅ Torrent engine pause/resume (`bridge.pause_all`/`resume_all`) değişmez (gerileme yok).
- ✅ Birim testleri: `api::tests::gap29_global_pause_flags_and_signals`,
  `api::tests::gap29_paused_loop_top_blocks_until_resume` (lib 20/20 geçti). Release binary derlendi.

## Not — bireysel pause/resume (per-task)

`/api/pause` + `task_id` (bireysel) yalnız torrent engine için geçerlidir (`bridge.pause_torrent`);
native HTTP-direct için bireysel pause Python'da da yoktur, dolayısıyla native dal yalnız global
pause/resume uygular. Bu kasıtlı bir kısıt değildir, Python parity'sidir.
