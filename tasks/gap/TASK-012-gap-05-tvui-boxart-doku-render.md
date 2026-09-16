---
id: TASK-012-gap-05
title: "TVUI box-art doku render eksigi — icon_path texture blit ve cache"
status: done
priority: P2
created: 2026-09-03
updated: 2026-09-03
environment: both
labels: [tvui, sdl2, frontend, boxart]
depends_on: [TASK-012h, TASK-012-gap-04]
---

# TASK-012-gap-05 — TVUI box-art doku render eksigi

## Tespit

- `manager-rs/manager-tvui/src/sdl2_shell.rs:192` `let _icon_path = BoxArtCache::icon_path_for(&p.name, &theme.icons.path);` — yol cozuluyor fakat `Texture` Yukleme/blit yok.
- `manager-rs/manager-tvui/src/render.rs:1` `BoxArtCache` yalnizca yol cache'liyor; SDL `TextureCreator` ile `load_texture` ve LRU cache yok; grid tile'lari duz renk kutu olarak kaliyor.
- Python `display/grid.py` ve `transitions.py` box-art yukleme davranisi Rust'a tasinmadi (Faz 5 iskelet).

## Kapsam

- `render.rs` `BoxArtCache` genislet: `HashMap<PathBuf, Texture>` veya `image` crate ile decode -> `Texture` olustur; eksik dosya icin fallback renk.
- `draw_grid` icinde `icon_path_for` sonucu `Texture` olarak yukle ve tile icine `canvas.copy()` ile blit et; secili tile `trans_scale` ile birlikte olcekle.
- Bellek siniri: cache cap (or. 64 texture) + LRU veya `icons.path` degisiminde invalidate.
- `cargo check` cross-platform dogrulamasi.

## Kabul Kriterleri

- [x] Icon dosyasi varsa grid tile icinde gorsel gorunuyor (`art.blit` tile rect'ine; secili tile `trans_scale` rect'iyle ayni); yoksa/bozuksa fallback renk kutu + etiket korunuyor (`blit=false` yolu).
- [x] Secili tile buyurken texture ile birlikte olcekleniyor (blit hedefi `(x,y,sw,sh)` olcekli rect).
- [x] Cache cap asilmiyor (`MAX_TEXTURES=64`, LRU evict; `cache_cap_evicts_oldest_with_dummy_video` testi); `icons.path` degisiminde invalidate (`sync_icons_path`).
- [x] `cargo check` (Linux) + `cargo test -p manager-tvui` 95/95 yesil (3 yeni: `decode_png_roundtrip`, `decode_garbage_returns_none`, cap+invalidate).
- [x] Windows cross (`--target x86_64-pc-windows-gnu`, j1) gecti (2026-09-16, 1m22s; tek uyarı pre-existing `state.rs:405`).
