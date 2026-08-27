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

## Mimari taslak (TASK-012-gap-01 inceleme dersleriyle, 2026-08-22)

İlke: **SDL'siz çekirdek + ince SDL kabuğu** — `ui_decision` deseninin devamı.
Karar/test edilebilir her şey saf modülde; SDL yalnız piksel işi yapar.

### Katmanlar

1. **`state.rs` (SDL'siz, test çekirdeği)**
   - `MenuState` enum: `Loading / PlatformGrid / GameList / Progress / Error / ConfirmExit…`
     (tvui.py `config.menu_state` değerlerinin tip-güvenli karşılığı).
   - `TvuiState` genişletme: mevcut alanlar + `selected_platform`, `games: Vec<GameRow>`,
     `selected_game`, `progress: HashMap<String, ProgressInfo>` (SSE history/progress kaynaklı).
   - SAF input reducer: `fn reduce(s: &mut TvuiScreen, key: UiKey)` — grid/game-list navigasyonu,
     sayfalama, confirm/back geçişleri. Bulgu 9'dan kalan nav/page tuşları burada tüketici bulur;
     **key-repeat** (Python `process_key_repeats`) reducer içinde timestamp ile.
   - Unit testler SDL'siz (scratch crate yöntemi, gap-01'de kanıtlandı).

2. **`render.rs`** — SDL2 çizim primitive'leri: rect, text (`sdl2-ttf`), texture cache
   (gap-01 Faz C bg-cache deseni genelleştirilir), image/box-art loader. Karar içermez.

3. **`screens.rs`** — saf state → çizim: loading bar (mevcut), platform_grid (mevcut draw_grid'in
   state-bağlı hali), game_list (durum sütunu parity), progress. Draw fonksiyonları yalnız state okur.

4. **i18n (gap-01 bulgu 11'in tüketici noktası)** — `t(key)` katmanı ilk metin çizimiyle birlikte:
   `languages/<lang>.json` okuyucu (`RGSX_TVUI_LANG` / rgsx_settings dili). String'ler artık
   `TriggerResult.message`'larda merkezî; UI string'leri buradan akar.

5. **Gamepad**: `gamepad_event_to_key` genişletilir (navup/down/left/right, pageup/down → reducer);
   SSE köprüsü ve shutdown bayrağı gap-01'de hazır.

6. **SSE**: watcher `handle_sse_frame`'e `progress`/`history` olayları eklenir → download_progress
   canlı akışı (Faz 11 contract yeniden kullanılır).

### Fazlar

- **Faz 1:** `state.rs` MenuState + reducer + TvuiState genişletme + SDL'siz unit testler.
- **Faz 2:** sdl2-ttf font yükleme (theme.json fonts alanı) + `t()` i18n okuyucu.
- **Faz 3:** loading + platform_grid ekranları, gamepad nav + key-repeat.
- **Faz 4:** game_list + progress ekranları, SSE progress/history olayları.
- **Faz 5:** transition efekti (`draw_validation_transition` parity) + box-art cache.

### Doğrulama stratejisi (bu sandbox gerçeği)

- Faz 1 (+ reducer/i18n/SSE-parse mantığı): izole scratch crate'te yeşil — SDL'siz.
- Faz 2-5 render dosyaları: ağır makine checklist'i (TASK-012-gap-01 dosyasındaki liste)
  tek geçişte; canlı gamepad→grid→game→download akışı Windows'ta doğrulanır.

---

## İlerleme

- 2026-08-21 — yön (B) kararıyla çıkarıldı (SPA TASK-012b superseded).
- 2026-08-22 — **Faz A (katalog OTA görünürlüğü) bu göreve katıldı** (kullanıcı onayı). Python
  parity: tvui.py `config.loading_progress` + `network.check_for_updates` → Rust'te SSE
  `catalog_update` (indirme ilerlemesi + `ready`) ile TVUI `loading` bar'ı. Backend plumbing
  `catalog_bootstrap.rs` içine eklendi (SSE publish + byte sayacı); `ensure_catalog_ready(None)`
  ile startup davranışı değişmedi. Self-update ayrı görev: TASK-012m.
- 2026-08-27 — **Faz 1 tamam** (`manager-tvui/src/state.rs` + `net.rs` UiKey genişletmesi, commit `c5ec3d1`): MenuState + TvuiScreen + SAF reducer (nav/page/confirm/back + key-repeat 120ms) + sync_from_net + 8 SDL'siz test (35/35 yeşil), `gamepad_event_to_key` nav/page eşlemesi.
