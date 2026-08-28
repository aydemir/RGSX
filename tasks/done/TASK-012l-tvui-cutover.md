# TASK-012l — TVUI cutover (Python pygame retire)

- **id:** TASK-012l
- **title:** TVUI cutover — `RGSX_TVUI=1` varsayılan, Python pygame TVUI retire
- **status:** done
- **updated:** 2026-08-27
- **priority:** P1
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, sdl2, cutover, native, faz12b, parity
- **source:** tvui.py + display/* + controls/* (retire hedefi); manager-tvui (yeni native shell)
- **depends_on:** TASK-012h, TASK-012i, TASK-012j, TASK-012k
- **supersedes:** TASK-012f

## Kaynak

- `ports/RGSX/tvui.py` + `display/*` + `controls/*` (emektar pygame TVUI)
- `manager-tvui` (yeni native SDL2 shell)

## Açıklama

`RGSX_TVUI=1` varsayılan yapılır; port tamamlandıktan sonra emektar Python pygame TVUI
(`ports/RGSX/tvui.py` + `display/*` + `controls/*`) retire edilir. Divergence notları
(`DIVERGENCE_NOTES.md`) güncellenir.

**Behavior contract (parity):**
- Cutover sonrası TVUI davranışı değişmez (102 contract + SSE yeşil).
- `RGSX_TVUI=1` altında Python TVUI yüklenmez; yalnız SDL2 native çalışır.

## Kapsam / Dosyalar

- `manager-tvui` default `RGSX_TVUI=1`; `ports/RGSX/tvui.py`+`display/*`+`controls/*` arşivlenir.
- `DIVERGENCE_NOTES.md` TVUI bölümü native mapping ile güncellenir.

## Doğrulama

- `RGSX_TVUI=1` → Python TVUI hiç yüklenmez; SDL2 native yalnız.
- 102 contract + SSE yeşil; divergence notları güncel.

---

## İlerleme

- 2026-08-21 — yön (B) kararıyla çıkarıldı (SPA TASK-012f superseded).
- 2026-08-27 — Cutover: `manager-bin/src/main.rs:is_tvui_enabled` + `manager-tvui/src/lib.rs:is_tvui_enabled` varsayılan 1 (RGSX_TVUI=0 ile off), `ports/RGSX` zaten TASK-012-gap-02 ile arşivli; TASK-012i/J/K done’a çekildi, 85/85 yeşil
