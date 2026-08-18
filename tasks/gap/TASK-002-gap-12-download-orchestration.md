# TASK-002-gap-12 — Download Orchestration (concurrency / slot / dedup / FIFO / global pause)

- **id:** TASK-002-gap-12
- **title:** Download orchestration layer (max_simultaneous_downloads gate, slot acquire/release, in-progress dedup, FIFO queue, global pause for HTTP-direct)
- **status:** done (tüm 5 madde; global-pause-HTTP-direct TASK-002-gap-29'da tamamlandı)
- **priority:** P1
- **created:** 2026-08-15
- **updated:** 2026-08-18
- **environment:** both
- **tags:** download, queue, concurrency, dedup
- **parent:** TASK-002

## Karar (güncellendi 2026-08-18 — bayatlık çözümü)

Orijinal `BELİRSİZ` iki karar **gap-1 retry motoru ile çözüldü** (2026-08-15 sonrası):

- **Concurrency gate mekanizması:** `tokio::sync::Semaphore` seçildi (AtomicUsize/Mutex
  sayacı DEĞİL). Kapasite `Settings.max_simultaneous_downloads`'tan türetilir:
  - Startup: `manager-bin/src/main.rs:283-286` (`Settings::load().max_simultaneous_downloads`).
  - Runtime ayar değişimi: `manager-http/src/api.rs:924-930` (`settings_post` semaphore'ı
    yeniden kurar).
  - İndirme anı: `api.rs:563` (torrent/bridge) ve `api.rs:1730` (`native_ddl_download`)
    `sem.acquire_owned().await` → izin task sonuna kadar tutulur (slot release = drop).
- **FIFO queue worker:** Ayrı worker/pop YERİNE, semaphore permit sırası (tokio FIFO)
  indirme başlangıç sırasını korur. `data.queue.push` yalnız SSE görüntüsü içindir.

`/api/download` ve `/api/download/batch` **request contract değişmedi.** Yeni internal
state: `StateData.download_semaphore` + `retry_in_flight: HashSet<String>` (in-progress
URL seti, dedup + bireysel pause/cancel için). Client'a yeni alan gönderilmedi.

## Python Referans Davranışı

- Concurrency limiti: `ports/RGSX/network/queue.py:97` (`max_dl = getattr(config,'max_simultaneous_downloads',5)`), `:99` (`if active < max_dl and config.download_queue:`)
- Slot acquire: `queue.py:101` (`config.active_download_count = active + 1`)
- Slot release: `queue.py:118-120` (`notify_download_finished()` decrement)
- Dedup: `queue.py:649-685` (`urls_in_progress` set + `urls_lock`; varsa `url_done_events` ile bekleyip cache sonucu döner)
- FIFO: `queue.py:100` (`job = config.download_queue.pop(0)`)
- Global pause: `queue.py:333-393` (`pause_all_downloads` / `resume_all_downloads` / `is_any_download_paused`)
- Bireysel pause/resume: `queue.py:288-311` (Rust'ta ✅ mevcut — `api.rs:1175-1215`)

## Rust Mevcut Durum (audit 2026-08-18)

- ✅ **Concurrency gate:** `StateData.download_semaphore` (Arc<Semaphore>), kapasite
  `main.rs:283-286` (startup) + `api.rs:924-930` (runtime ayar) ile `max_simultaneous_downloads`'tan.
  İndirme başına `acquire_owned().await` (`api.rs:563`, `api.rs:1730`). N sınırı geçilmez.
- ✅ **Slot acquire/release:** Semaphore izni task scope'unda tutulur, drop'ta otomatik release.
  `active: bool` ayrı bir görüntü bayrağıdır (state.rs:38).
- ✅ **Dedup (in-progress):** `claim_in_flight()` helper'ı (`api.rs`) `retry_in_flight` set'ine
  atomik check-and-insert yapar; aynı URL ikinci kez istenirse spawn düşürülür (torrent/bridge
  yolu `api.rs:~549`, native DDL yolu `native_ddl_download` `api.rs:~1716`). Completion'da
  `retry_in_flight.remove()` (`api.rs:669`, `api.rs:1847`). Birim testi:
  `api::tests::gap12_claim_in_flight_dedups_same_url`.
- ✅ **FIFO:** Semaphore permit sırası (tokio FIFO) ile sağlanır — dolu gate'te bekleyen
  indirmeler çıkış sırasıyla başlar. Ayrı worker gerekmedi.
- ✅ **Global pause (HTTP-direct):** `pause`/`resume` handler'larının native (bridge yok) dalı
  artık placeholder `queue_size` döndürmez — `global_paused` bayrağı + `pause_signals` (devam
  eden indirmeleri `CancelFlag` ile abort) ve `pause_resume` (bekleyen döngüleri uyandırır)
  mekanizması ile native HTTP-direct indirmeleri de duraklatır/sürdürür. **TASK-002-gap-29'da tamamlandı.**

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` — `claim_in_flight()` (dedup), `download()`/`native_ddl_download()`
  önüne dedup kapısı; semaphore gate zaten mevcut.
- `manager-rs/manager-core/src/state.rs` — `StateData.download_semaphore`, `retry_in_flight`.
- `manager-rs/manager-bin/src/main.rs:283-286` — startup semaphore kapasitesi (ayardan).
- `manager-rs/manager-http/tests/contract.rs` — dedup/gate contract testleri (ileride eklenebilir).

## Bağımlılık / Bölünme

- **TASK-002-gap-1** (retry engine): gate + dedup + slot mekanizması bu görevle aynı
  `tokio::spawn` akışını değiştirdi; gap-1 tamamlandıktan sonra ele alındı (✅).
- **TASK-002-gap-29** (global pause HTTP-direct): global pause'ın yalnız torrent engine'i
  kapsaması, native HTTP-direct'i kapsamaması ayrı, daha büyük bir özelliktir
  (native HTTP indirme döngüsünün pause flag'ini onurlandırması gerekir). gap-12'den
  ayrıldı; kendi görev dosyasında takip edilir.

## Doğrulama

- ✅ `max_simultaneous_downloads=N` iken eşzamanlı aktif indirme N'yi geçmez (semaphore gate).
- ✅ Aynı URL 2. kez istenirse ikinci istek spawn edilmez (dedup, corrupt/partial çakışma yok).
- ✅ Kuyruk FIFO sırası korunur (semaphore permit sırası).
- ✅ Global pause HTTP-direct indirmeyi duraklatır (TASK-002-gap-29 tamamlandı).
- `tests/test_download_batch.py` senaryoları Rust'ta test edilir (contract.rs) — dedup/gate
  birim testi `gap12_claim_in_flight_dedups_same_url` mevcut.
