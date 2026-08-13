# TASK-002m — Faz 10c: librqbit canlı indirme progress akışı (WebUI)

- **id:** TASK-002m
- **title:** librqbit indirme sırasında WebUI'ye canlı progress yayar (handler callback → state.progress + SSE)
- **status:** done
- **priority:** P1
- **created:** 2026-08-13
- **environment:** both
- **tags:** rust, librqbit, faz-10c, progress, sse

## Kaynak

- Analiz (oturum 2026-08-13): "qBittorrent progress vs WebUI progress" sorunu — kök neden #1
  (regresyon). librqbit varsayılan motor olunca `LibrqbitEngine::download_torrent_source`
  yalnız `add → wait_until_completed → resolve` yapıyordu; **arada hiç progress callback'i
  yoktu**. Eski qBittorrent yolu `progress_queue` ile canlı akıtıyordu; librqbit yolunda bu
  yoktu → WebUI progress barı 0%/"Downloading"de takılı, bitince aniden tamamlanıyordu.
- `manager-torrent/src/lib.rs` — `LibrqbitEngine::download_torrent_source` (eski).
- `manager-bridge/src/lib.rs` — `TorrentBackend` trait (`download_torrent` varsayılanı).
- `manager-http/src/api.rs` — `download()` spawn task'ı (eski: `bridge.download_torrent`).

## Kök Neden

`TorrentBackend::download_torrent` sözleşmesi yalnız **sonuç yolu** (PathBuf) döndürür;
indirme **sırasında** ilerleme aktarmaz. Python bridge bunu kendi iç `progress_queue`
akışıyla çözer (WebUI proxy ile okur), ama librqbit engine'de eşdeğer canlı akış
yoktu. `wait_until_completed()` tek bir await olduğundan handler bir "tamamlandı"
sonucu alana dek UI'a hiç ara durum yazmıyordu.

## Davranış Kuralları

1. `TorrentBackend` sözleşmesine `download_torrent_progress(source, dest, on_progress)`
   eklenir; varsayılan impl `on_progress`'u yok sayıp `download_torrent`'e düşer
   (Python bridge davranışı korunur).
2. `LibrqbitEngine` override eder: `handle.stats()` döngüsünden (`progress_bytes`,
   `total_bytes`, `live.download_speed`, `finished`) `ProgressEvent` yayar; bitince
   `wait_until_completed` ile hash-check tamamlanır.
3. `api.rs` `download()` spawn task'ı `download_torrent_progress` çağırır; callback
   `state.progress[game_url] = {status, progress%, downloaded, total, speed}` yazar ve
   SSE `progress` olayını yayar. `finalize_download_in_state` bitişte 100/`Download_OK` yazar.
4. `ProgressEvent` yeni ortak tip: `{downloaded:u64, total:u64, speed:f64, finished:bool}`.

## Kapsam / Dosyalar

- `manager-bridge/src/lib.rs` — `ProgressEvent` + `download_torrent_progress` (varsayılan).
- `manager-torrent/src/lib.rs` — `download_torrent_source_with_progress` + `LibrqbitEngine` override.
- `manager-http/src/api.rs` — `download()` callback + `state.progress`/SSE güncelleme.
- `manager-http/tests/contract.rs` — `test_download_streams_progress_callback` (offline).
- `manager-http/tests/live_download.rs` — zaten `#[ignore]` canlı Sintel (artık progress de akar).

## Doğrulama

- `cargo test -p manager-http --test contract` → 102 passed (yeni progress testi dahil).
- `cargo test -p manager-torrent` → 9 passed (mevcut engine testleri yeşil).
- Canlı (Windows): `RGSX_TORRENT_ENGINE=librqbit` + magnet ile indirme → WebUI progress
  barı canlı artar (0→100), qBittorrent penceresiyle aynı akışı gösterir.

---

## İlerleme

- 2026-08-13 — kök neden + davranış kuralları; implementasyon tamam (ProgressEvent,
  trait metodu, librqbit polling, api.rs callback + SSE); 102 contract + 9 engine testi yeşil.
