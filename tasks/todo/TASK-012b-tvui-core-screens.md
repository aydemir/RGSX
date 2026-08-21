# TASK-012b — TVUI Çekirdek Ekranlar (Core Screens)

- **id:** TASK-012b
- **title:** TVUI çekirdek ekranlar — loading / platform_grid / game_list / download_progress
- **status:** todo
- **priority:** P0
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, screens, parity, faz12b

## Kaynak

- `plan.md` §5.2, `ports/RGSX/tvui.py` (`draw_loading_screen`, `draw_platform_grid`,
  `draw_game_list`, `draw_progress_screen`), `display/screens.py`, `display/grid.py`,
  `display/progress.py`
- `docs/roadmap/FAZ12_PARITY_STRATEGY.md`
- `plan-ui-live-queue.md` ADIM 1 (SSE canlı bağlantı — kodda zaten tam)

## Açıklama

Eski pygame çizimleri SPA bileşenlerine taşınır (impl serbest). Kullanıcının gördüğü
**davranış** parity'de kalır.

**Behavior contract (parity):**
- `loading` → boot ekranı görünür.
- `platform_grid` → sistem/platform ızgarası; seçim → `game_list` açılır (aynı sıralama/filtre).
- `game_list` → oyun listesi; durum (indirildi/indiriliyor) eskiyle aynı görünür.
- `download_progress` → aktif indirme ilerleme; SSE `queue`/`progress` ile canlı akar
  (yeniden refresh gerektirmez).

## Kapsam / Dosyalar

- `webui/` — ilgili SPA ekran bileşenleri (`?mode=tv`).
- `manager-tvui/src/native_input.rs` — gamepad navigasyon (zaten portlu, yeniden kullanılır).
- `manager-http/src/sse.rs` — canlı event (zaten var).

## Doğrulama

- Gamepad ile platform→oyun→indirme akışı canlı çalışır; ilerleme SSE ile akar.
- "Aynı input → aynı görünür state" contract testi (snapshot/SSE state diff).
- 102 contract baseline yeşil.

---

## İlerleme

- 2026-08-21 — plan.md §5.2'den çıkarıldı.
