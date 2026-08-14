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
