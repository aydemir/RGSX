# TASK-012h — Render core + ekran state machine (çekirdek ekranlar)

- **id:** TASK-012h
- **title:** SDL2 render core + `menu_state` state machine (loading/platform_grid/game_list/progress)
- **status:** in-progress
- **priority:** P0
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, sdl2, render, native, faz12b, parity
- **source:** tvui.py (main loop — config.menu_state dispatch); display/screens.py, grid.py, game_list.py, progress.py, core.py, components.py, background.py
- **depends_on:** TASK-012g
- **supersedes:** TASK-012b

## Kaynak

- `tvui.py` ana döngüsü (`config.menu_state` dispatch: loading → platform_grid → game_list → progress)
- `display/background.py`, `screens.py`, `grid.py` (28KB), `game_list.py` (26KB), `progress.py`, `core.py`, `components.py`
- `transitions.py` (`draw_validation_transition` → SDL2 scale+alpha+neon efekti)
- `manager-http/src/sse.rs` (Faz 11 canlı ilerleme, yeniden kullanılır)

## Açıklama

`tvui.py` ana döngüsündeki `config.menu_state` state machine'i native SDL2'ye portlanır.
pygame `draw_*` fonksiyonları SDL2 primitives'e çevrilir: rect / text+font / image+box-art /
transition. Canlı SSE ilerleme yeniden kullanılır.

**Behavior contract (parity):**
- Açılışta loading ekranı anında görünür (siyah ekran yok).
- platform_grid → game_list geçişi (aynı sıralama/filtre); game_list durum sütunu aynı.
- download_progress SSE ile canlı güncellenir (Faz 11 contract).

## Kapsam / Dosyalar

- `manager-tvui/src/render.rs` (SDL2 çizim primitive'leri), `screens.rs` (loading/platform_grid/
  game_list/progress draw), `state.rs` (`menu_state` dispatch + main loop).
- `native_input.rs` gamepad navigasyonu yeniden kullanılır.
- Kutu-artı (box-art) görsel önbelleği: `display/` ikon/art yükleme mantığı portlanır.

## Doğrulama

- Gamepad ile platform → game → download akışı canlı; SSE ilerleme akar.
- 102 contract (loading/platform_grid/game_list/progress) yeşil.

---

## İlerleme

- 2026-08-21 — yön (B) kararıyla çıkarıldı (SPA TASK-012b superseded).
- 2026-08-22 — **Faz A (katalog OTA görünürlüğü) bu göreve katıldı** (kullanıcı onayı). Python
  parity: tvui.py `config.loading_progress` + `network.check_for_updates` → Rust'te SSE
  `catalog_update` (indirme ilerlemesi + `ready`) ile TVUI `loading` bar'ı. Backend plumbing
  `catalog_bootstrap.rs` içine eklendi (SSE publish + byte sayacı); `ensure_catalog_ready(None)`
  ile startup davranışı değişmedi. Self-update ayrı görev: TASK-012m.
