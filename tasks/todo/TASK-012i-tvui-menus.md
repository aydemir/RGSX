# TASK-012i — Menüler (pause / display / filter / sort / search)

- **id:** TASK-012i
- **title:** SDL2 menüler (pause_menu / display_menu / filter_* / global_sort / global_search)
- **status:** in-progress
- **updated:** 2026-08-27
- **priority:** P2
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, sdl2, menu, native, faz12b, parity
- **source:** display/menus.py (92KB), display/global_search.py; controls/menus.py, controls/search.py; game_list.py (filtering)
- **depends_on:** TASK-012h
- **supersedes:** TASK-012c

## Kaynak

- `display/menus.py` (92KB), `display/global_search.py`
- `controls/menus.py`, `controls/search.py`
- `game_list.py` (filtre/sıralama mantığı)

## Açıklama

`display/menus.py` + `global_search.py` + `controls/menus.py` + `controls/search.py` SDL2'ye
portlanır: pause_menu, display_menu, filter_*, global_sort_menu, global_search. Buton
etiketleri + gamepad navigasyon parity'si; filtre/sıralama davranışı aynı.

**Behavior contract (parity):**
- pause → tüm indirmeler durur; resume → devam eder (SSE state).
- display_menu: tema/font canlı değişir.
- filter/sort/search: eski TVUI ile aynı sonuç kümesi + sıralama.

## Kapsam / Dosyalar

- `manager-tvui/src/menus.rs` (pause/display/filter/sort), `search.rs` (global_search + folder_search).
- Etiket metinleri `language.py` i18n ile eşleşir (Faz 9).

## Doğrulama

- Gamepad navigasyon + buton etiketleri eski TVUI ile eşleşir.
- pause/resume SSE state doğrulanır; filtre/sort/search sonuç parity'si.

---

## İlerleme

- 2026-08-21 — yön (B) kararıyla çıkarıldı (SPA TASK-012c superseded).
- 2026-08-27 — Faz 1: `menus.rs` SDL'siz state machine (pause/display/filter/sort/search keys + i18n, 6 test, 52/52) commit `f2a63b9`.
- 2026-08-27 — Faz 2: overlay entegrasyonu (`state.rs` overlay + `sdl2_shell.rs` draw_menu_overlay + M/Menu key + nav, 53/53) commit `5be5131`.
- 2026-08-27 — Faz 3: filter/sort tamam — `state.rs:filtered_games` (region filter + numeric size parse + sort_mode), 54/54 yeşil, commit `89045c2`.
