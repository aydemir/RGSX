# TASK-002-gap-12 — Download Orchestration (concurrency / slot / dedup / FIFO / global pause)

- **id:** TASK-002-gap-12
- **title:** Download orchestration layer (max_simultaneous_downloads gate, slot acquire/release, in-progress dedup, FIFO queue, global pause for HTTP-direct)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** download, queue, concurrency, dedup
- **parent:** TASK-002

## Karar (2026-08-15)

`/api/download` ve `/api/download/batch` **request contract değişmez.** Yeni internal state
(current active count, in-progress URL set, FIFO queue) eklenir; client'a yeni alan gönderilmez.
İndirme başlatma kapısı `manager-http/src/api.rs` içindeki `tokio::spawn` akışına konur
(TASK-002-gap-1 retry motoru ile AYNI nokta — bkz. Bağımlılık).

> BELİRSİZ: concurrency gate mekanizması — `tokio::sync::Semaphore` mı yoksa `StateData`
> içinde `AtomicUsize`/`Mutex<usize>` slot sayacı mı kullanılacak? Uygulamaya başlamadan
> kullanıcı onayı gerekir. Ayrıca FIFO queue worker'ı ayrı bir tokio task mı yoksa mevcut
> spawn içinde pop mantığı mı — BELİRSİZ.

## Python Referans Davranışı

- Concurrency limiti: `ports/RGSX/network/queue.py:97` (`max_dl = getattr(config,'max_simultaneous_downloads',5)`), `:99` (`if active < max_dl and config.download_queue:`)
- Slot acquire: `queue.py:101` (`config.active_download_count = active + 1`)
- Slot release: `queue.py:118-120` (`notify_download_finished()` decrement)
- Dedup: `queue.py:649-685` (`urls_in_progress` set + `urls_lock`; varsa `url_done_events` ile bekleyip cache sonucu döner)
- FIFO: `queue.py:100` (`job = config.download_queue.pop(0)`)
- Global pause: `queue.py:333-393` (`pause_all_downloads` / `resume_all_downloads` / `is_any_download_paused`)
- Bireysel pause/resume: `queue.py:288-311` (Rust'ta ✅ mevcut — `api.rs:850-882`)

## Rust Mevcut Durum (❌)

- `manager-core/src/settings.rs:113` `max_simultaneous_downloads` TANIMLI (default 5) ama hiçbir yerde uygulanmıyor.
- `manager-http/src/api.rs:511` `tokio::spawn` KOŞULSUZ — `active < max_dl` kapısı yok (grep: `active_download_count`/`Semaphore` Rust'te 0 eşleşme).
- Slot: `manager-core/src/state.rs:32` yalnız `active: bool`; tamsayı sayaç / acquire-release YOK.
- Dedup: `manager-http/src/api.rs:478-573` `download()` URL zaten in-progress mı diye bakmadan spawn+push (grep: `urls_in_progress`/`dedup`/`already` api.rs'te yok).
- FIFO: `api.rs:567` `data.queue.push(...)` yalnız log; ayrı worker/pop yok, indirme anında başlar.
- Global pause: `api.rs:857`→`manager-bridge/src/lib.rs:363-365`→`manager-torrent/src/lib.rs:225-238` (yalnız torrent engine); placeholder dalı `api.rs:861` yalnız `queue_size` döndürür, HTTP-direct'i kapsamaz.

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-rs/manager-http/src/api.rs` — `download()`/`batch` önüne concurrency gate + dedup; FIFO worker; global pause HTTP-direct dalı
- `manager-rs/manager-core/src/state.rs` — `StateData` içi slot sayacı + `in_progress_urls: HashSet`
- `manager-rs/manager-core/src/settings.rs:113` — mevcut ayar zaten var, gate'e bağlanır
- `manager-rs/manager-http/tests/contract.rs:1445` — batch dedupe/sayaç/kick testi EKLENMELİ (şu an yalnız proxy)

## Bağımlılık

- **TASK-002-gap-1** (retry engine) — her iki görev `api.rs` içindeki aynı `tokio::spawn` download
  akışını değiştirir. gap-1 önce bitmeli VEYA tek bir ortak refactor PR'ında birleştirilmeli.
  Çakışmayı önlemek için gap-1 tamamlandıktan sonra ele alınması önerilir.

## Doğrulama

- `max_simultaneous_downloads=N` iken eşzamanlı aktif indirme sayısı N'yi geçmez (test: N=2, 5 URL).
- Aynı URL 2. kez kuyruğa girerse ikinci sıra ilk bitince sonuçlanır (dedup, partial/corrupt çakışma yok).
- Kuyruk FIFO sırası korunur; global pause HTTP-direct indirmeyi de duraklatır.
- `tests/test_download_batch.py` senaryoları Rust'ta test edilir (contract.rs).
