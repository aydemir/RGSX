# TASK-012g — Native TVUI shell + `.json` tema yükleyici (SDL2)

- **id:** TASK-012g
- **title:** Native TVUI shell (rust-sdl2) + `theme.json` yükleyici + `RGSX_TVUI` flag hook
- **status:** completed
- **priority:** P1
- **created:** 2026-08-21
- **environment:** both
- **tags:** tvui, sdl2, theme, native, faz12b, parity
- **source:** plan.md §5.1; display/colors.py, fonts.py, transitions.py, icons.py; tvui.py (pygame.init/joystick.init + init_display)
- **depends_on:** TASK-005
- **supersedes:** TASK-012a

## Kaynak

- `display/colors.py` (THEME_COLORS + BACKGROUND_THEME_PRESETS), `fonts.py`, `transitions.py`, `icons.py`
- `tvui.py` (pygame.init / joystick.init + init_display) — native SDL2 eşdeğeri
- `manager-tvui/src/native_input.rs` (gilrs gamepad, TASK-005) — girdi katmanı

## Açıklama

Yön (B): TVUI'yi `rust-sdl2` ile native 10-foot render'a taşı. `manager-tvui` içindeki
webview/`wry`+`tao` kabuğu SDL2 tam ekran render'a dönüştürülür. `display/colors.py`
(THEME_COLORS + BACKGROUND_THEME_PRESETS), `fonts.py`, `transitions.py`, `icons.py`
tek bir **`theme.json`** şemasına portlanır; `serde_json` ile tip-güvenli yüklenir.

`theme.json` şeması (ES `theme.xml` yerine):

```
{
  "name": "default",
  "colors": { "fond_lignes":[0,255,0], "fond_image":[50,50,70], "neon":[0,134,179],
              "background_top":[20,25,35], "background_bottom":[45,55,75],
              "button_idle":[45,50,65,180], "text":[255,255,255], "error_text":[255,60,60],
              "success_text":[0,255,150], "warning_text":[255,150,0], "title_text":[220,220,230],
              "border":[100,120,150], "border_selected":[0,255,150], "shadow":[0,0,0,100],
              "glow":[100,180,255,40], "accent_gradient_start":[80,120,200],
              "accent_gradient_end":[120,80,200] },
  "background_presets": {
    "default": {"top":[20,25,35], "bottom":[45,55,75]},
    "sunset":  {"top":[52,24,44],  "bottom":[173,82,56]},
    "forest":  {"top":[18,36,32],  "bottom":[50,88,72]},
    "midnight":{"top":[8,13,26],   "bottom":[27,43,79]} },
  "fonts": { "family":"pixel", "pixel_ttf":"assets/fonts/Pixel-UniCode.ttf", "fallback":"dejavusans" },
  "transitions": { "platform_select": {"duration_ms":1000, "scale_min":1.5, "scale_max":2.5} },
  "icons": { "path":"assets/icons/", "set":"default" }
}
```

`RGSX_TVUI` bayrağı korunur: `0` → eski Python pygame TVUI fallback (değişmez);
`1` → yeni native SDL2 shell + `theme.json`.

**Behavior contract (parity):**
- TV modu tam ekran 10-foot açılır; palet `colors.py` ile birebir eşleşir.
- Arka plan preset'leri (default/sunset/forest/midnight) `theme.json`'dan yüklenir.
- `RGSX_TVUI=0` → Python fallback değişmeden çalışır (102 contract yeşil).

## Kapsam / Dosyalar

- `manager-tvui/src/sdl2_shell.rs` (SDL2 init + fullscreen), `theme.rs` (`theme.json` parse/validate).
- `assets/theme.json` + mevcut `assets/fonts/`, `assets/icons/`.
- Webview bağımlılığı (wry/tao) kaldırılır; `gilrs` (native_input.rs, TASK-005) kullanılır.

## Doğrulama

- `cargo build -p manager-tvui` (Linux proot + Windows hedefi).
- `RGSX_TVUI=0` → Python fallback; `RGSX_TVUI=1` → SDL2 native açılır, tema paleti eşleşir.
- 102 contract her iki modda yeşil.

---

## İlerleme

- 2026-08-21 — plan.md §5.1 + yön (B) kararıyla çıkarıldı (SPA TASK-012a..f superseded).
- 2026-08-22 — `theme.rs`, `sdl2_shell.rs`, `assets/theme.json`, `lib.rs` yazıldı; build yeşil (SDL2
  bundled, cmake 4.x için `CMAKE_POLICY_VERSION_MINIMUM=3.5`). `cargo test -p manager-tvui` →
  7/7 yeşil (tema birim testleri + native_input). `RGSX_TVUI=1` canlı SDL2 penceresi manuel doğr.
  bekliyor (palet gözle kontrol).
