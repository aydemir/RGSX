# TASK-002-gap-29 — Global pause for HTTP-direct downloads

- **id:** TASK-002-gap-29
- **title:** Global pause/resume must also halt native HTTP-direct downloads (not only the torrent engine)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-18
- **environment:** both
- **tags:** download, pause, http-direct, orchestration
- **parent:** TASK-002

## Bağlam (TASK-002-gap-12'den ayrıldı)

`TASK-002-gap-12` download orchestration'ın çekirdek 4 maddesi (concurrency gate, slot
acquire/release, in-progress dedup, FIFO) tamamlandı. Kalan tek madde **global pause'ın
yalnız torrent engine'i kapsaması, native HTTP-direct indirmeyi kapsamaması** ayrı, daha
büyük bir özelliktir ve kendi görevine ayrıldı.

## Python Referans Davranışı

- `ports/RGSX/network/queue.py:333-393` — `pause_all_downloads()` / `resume_all_downloads()` /
  `is_any_download_paused`. Global duraklatma aktifken yeni indirmeler başlamaz ve devam eden
  HTTP indirmeleri (stream) durdurulur; resume'da kaldığı yerden devam eder.

## Rust Mevcut Durum (❌)

- `manager-http/src/api.rs:1175-1215` (`pause`/`resume` handler'ları): `bridge` (librqbit /
  torrent engine) varsa `bridge.pause_all()` / `resume_all()` çağrılır ve torrent indirmeleri
  duraklatılır.
- `bridge` yoksa (native HTTP-direct modu) handler yalnız `state.read().queue_size()` döndürür
  — **placeholder**, gerçekten duraklatmaz.
- Native HTTP indirme döngüsü `native_ddl_download` (`api.rs:~1727`) içinde
  `manager_download::http::HttpDownloader` ile stream eder; döngü bir global "paused" flag'ini
  onurlandırmaz. `HttpDownloader` zaten `with_cancel`/`CancelFlag` destekler — global pause
  bu mekanizmaya bağlanmalı (pause = cancel flag set + yeniden başlatma engeli; resume = devam).

## Kapsam / Dosyalar (değişecek)

- `manager-rs/manager-http/src/api.rs` — `pause`/`resume` handler'ları native modda global
  "paused" durumu set/reset eder; `native_ddl_download` döngüsü bu flag'i sorgular.
- `manager-rs/manager-core/src/state.rs` — `StateData` içi `global_paused: bool` (veya
  `Arc<Notify>`/atomic) alanı eklenir.
- `manager-rs/manager-download/src/http.rs` — `HttpDownloader` stream'i pause noktasında
  global flag'i kontrol eder (cancel flag ile birleştirilir).

## Doğrulama

- Global pause aktifken devam eden native HTTP-direct indirme durur; queue yeniden başlamaz.
- Resume'da indirme kaldığı yerden devam eder (Range resume zaten mevcut).
- Torrent engine pause/resume davranışı değişmez (gerileme yok).
- Contract/unit test: global pause altında yeni HTTP-direct indirmenin başlamaması doğrulanır.
