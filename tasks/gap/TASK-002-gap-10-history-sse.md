# TASK-002-gap-10 — History / State-Emitter / SSE Sonlandırma (Rust daemon'da eksik)

- **id:** TASK-002-gap-10
- **title:** History & state-emitter / SSE finalization in daemon (mark_game_as_downloaded, emit, bulk status)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** history, sse, state-emitter, download
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `F0` (kısmi), `D2`, `D7`
- `ports/RGSX/network/queue.py`:
  - `_finalize_download_result`: `mark_game_as_downloaded`, `emit_state_event` (completed/retry/failed),
    `_set_bulk_history_status`, `_update_history_local_target`
  - `download_rom`: per-5% history kaydı (`_save_history_with_feedback`), `config.needs_redraw`
- `ports/RGSX/network/download_state.py`: `set_state_emitter`, `emit_state_event` (SSE 'download_state')
- `ports/RGSX/rgsx_manager.py`: `set_state_emitter(_broadcast)` (Faz 8 SSE yayını)

## Açıklama

Python, indirme sonucunu history JSON'a yazar, `mark_game_as_downloaded` ile kitaplığı işaretler
ve SSE `download_state` eventi yayar. `rust_daemon.download_torrent` yalnızca
`_mirror_progress` ile progress'i yansıtır; **tamamlanma sonrası history sonlandırma,
mark_game_as_downloaded ve emit_state_event Rust daemon'da yoktur**. Şu an bu, Python delegate
sarmalayıcısı (`_finalize_download_result`) tarafından hâlâ yapılıyor, ama Python kaldırıldığında
kaybolur.

## Kapsam / Dosyalar

- `manager-rs/manager-bin/src/` — `/api/download` tamamlanınca history yazma + SSE emit sözleşmesi
- `manager-rs/manager-core/src/state.rs` — `emit_state_event` Rust karşılığı (zaten enum/transition var)
- `rust_daemon.py` — `_mirror_progress` sonlandırma adımıyla genişletme

## Doğrulama

- Rust torrent tamamlanınca history entry "Download_OK" + `mark_game_as_downloaded` güncellenir.
- SSE `download_state` eventi (completed/retry/failed) yayılır, WebUI/TVUI parity.
- Per-5% progress kaydı korunur (gereksiz yazma olmadan).

---

## Parite Denetimi 2026-08-15 — Ek Maddeler

Parity denetimi bu TASK'ın kapsamını genişletti. Aşağıdaki maddelere de bu TASK altında bakılacak.

### Madde A: history.json disk kalıcılığı tamamen yok (❌ — veri kaybı riski)

- Python: `history.py:358-373` init, `:375-420` load (geçersiz entry filtreleme), `:195-220` atomic
  write (temp+fsync+replace, 5 retry), `:422-443` throttle/batch save, `:81-193` write-failure
  cooldown/probe.
- Rust: `manager-http/src/api.rs:227` `state.read().history.clone()` (yalnız bellek);
  `:575,1143` belleğe push; disk flush YOK. Native mod yalnız bellek — Python kaldırılınca geçmiş kaybolur.

### Madde B: clear_history aktif indirmeyi de siler (❌ — veri/indirme kaybı)

- Python: `history.py:463-509` `clear_history` aktif indirmeleri KORUR (Downloading/Extracting/
  Seeding/Queued+aktif id/url).
- Rust: `manager-http/src/api.rs:775` `state.write().history.clear()` — **tüm entry'leri siler**,
  aktif koruma YOK.

### Madde C: downloaded_games.json + ROM tarama yok (❌)

- Python: `history.py:626-783` `downloaded_games.json` persist, `mark_game_as_downloaded`, ROM
  klasörü taraması.
- Rust: `manager-core/src/state.rs:29-30,50` yalnız bellek `json!({})`; persist/tarama yok.

### Madde D: add_to_history timestamp otomatik üretilmiyor (⚠️ KISMİ)

- Python: `history.py:454` `timestamp` otomatik üretilir.
- Rust: `api.rs:582,1150` `timestamp` hep `""` (otomatik üretilmez, tam metin history.json'da korunur).

### Madde E: Progress throttle / SSE frekansı yok (⚠️ KISMİ)

- Python: `http_download.py:243,258` (0.1s stream emit), `one_fichier.py:1309` (%5 history persist
  throttle), SSE `_broadcaster_loop` ~250ms.
- Rust: `manager-http/src/api.rs:517-543` her bridge event'inde SSE yayar, açık throttle yok
  (librqbit event hızına bağımlı — SSE seli riski).

### Bağımlılık

- `TASK-002-gap-1` (retry engine) — history sonlandırma `api.rs` `finalize_download_in_state`
  (paylaşılan nokta) üzerinden yazılır; gap-1 ile çakışmamak için sıralı ele alınmalı.
