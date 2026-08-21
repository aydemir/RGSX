# TASK-012c — TVUI Menüler (Pause / Display / Filter / Sort / Search)

- **id:** TASK-012c
- **title:** TVUI menü ekranları — pause_menu, display_menu, filter_*, global_sort_menu, global_search
- **status:** todo
- **priority:** P2
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, menus, parity, faz12b

## Kaynak

- `plan.md` §5.3, `ports/RGSX/tvui.py` (`draw_pause_menu`, `draw_display_menu`,
  `draw_filter_menu_choice`, `draw_filter_advanced`, `draw_filter_priority_config`,
  `draw_global_sort_menu`, `draw_global_search_list`), `display/menus.py`,
  `display/filter.py`, `controls/search.py`

## Açıklama

Menü ekranları SPA'ya taşınır; buton etiketleri ve navigasyon şeması **parity'de kalır**
(kullanıcı görür). pygame `draw_*` → SPA modal/bileşen (impl serbest).

**Behavior contract (parity):**
- `pause_menu`: duraklat → tüm aktif HTTP+torrent durur; sürdür → kaldığı yerden (Faz parity).
- `display_menu`: tema/font ayarı anında yansır.
- `filter_*`: filtre seçenekleri ve sonuç davranışı eskiyle aynı.
- `global_sort_menu` / `global_search`: sıralama + arama davranışı aynı.

## Kapsam / Dosyalar

- `webui/` — menü bileşenleri (`?mode=tv`).
- `controls/menus.py` + `controls/search.py` davranışı SPA state'e çevrilir.

## Doğrulama

- Her menüde gamepad navigasyon + buton etiketleri eskiyle eşleşir (görsel/contract).
- Pause/resume davranışı SSE state ile doğrulanır (aynı state geçişi).

---

## İlerleme

- 2026-08-21 — plan.md §5.3'den çıkarıldı.
