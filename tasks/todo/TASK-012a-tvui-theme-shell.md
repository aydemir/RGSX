# TASK-012a — TVUI Tema Altyapısı + `?mode=tv` Shell + Flag Hook

- **id:** TASK-012a
- **title:** TVUI SPA shell (CSS tema değişkenleri) + `RGSX_TVUI` flag hook
- **status:** superseded
- **superseded_by:** Yön (B) native SDL2 + `.json` tema — bkz. TASK-012g..l (2026-08-21)
- **priority:** P1
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, theme, parity, faz12b

## Kaynak

- `plan.md` §5.1, `docs/roadmap/ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` §0/§12b
- `docs/roadmap/FAZ12_PARITY_STRATEGY.md` (davranış parity'si zorunlu, yapı serbest)

## Açıklama

Eski `display/colors.py` + `fonts.py` + `background.py` + `icons.py` + `transitions.py`
pygame tabanlı tema katmanının **görünümü** (renk paleti, font ölçekleme hissi, yerleşim)
SPA'ya CSS değişkenleri olarak taşınır. `?mode=tv` 10-foot layout shell'i kurulur.
`RGSX_TVUI=1` flag'i ile eski Python TVUI fallback korunur (cutover'a kadar).

**Behavior contract (parity):**
- TV modu açıldığında ekran tam ekran 10-foot düzen; renk/font paleti eski `colors.py`
  ile görsel olarak eşleşir.
- Flag kapalıyken eski Python TVUI davranışı değişmez (regression yok).

## Kapsam / Dosyalar

- `webui/` — `?mode=tv` shell + `theme.css` (eski palet değişkenleri), kiosk CSS.
- `manager-tvui/src/` — `launch()` webview/SPA açar; `RGSX_TVUI` okuma hook'u.
- `manager-http/src/catalog.rs` / static serve — SPA kök `RGSX_TVUI` ile `?mode=tv` yönlenir.

## Doğrulama

- `RGSX_TVUI=0` → Python TVUI fallback (mevcut davranış) korunur.
- `RGSX_TVUI=1` → SPA `?mode=tv` açılır, tema paleti eski görünümle eşleşir (görsel diff).
- Contract testleri (102 baseline) iki modda da yeşil.

---

## İlerleme

- 2026-08-21 — plan.md §5.1'dan çıkarıldı.
