---
id: TASK-012-gap-04
title: "TVUI frontend TTF metin render eksigi — sdl2_ttf entegrasyonu ve overlay etiketleri"
status: done
priority: P1
created: 2026-09-03
updated: 2026-09-16
environment: both
labels: [tvui, sdl2, frontend, ttf]
depends_on: [TASK-012h, TASK-012i, TASK-012j, TASK-012k]
---

# TASK-012-gap-04 — TVUI frontend TTF metin render eksigi

## Tespit (codegraph + sdl2_shell.rs dogrulamasi)

- `manager-rs/manager-tvui/src/sdl2_shell.rs:113` `draw_loading` — hata/normal bar sadece `fill_rect`/`draw_rect`; yorum: `metin yok — TTF erte`.
- `sdl2_shell.rs:159` `draw_grid` — `platforms` tile'lari `button_idle`/`button_selected` kutu; `PlatformTile.name` hic cizilmiyor; `_icon_path` hesaplanip atiliyor.
- `sdl2_shell.rs:228` `draw_game_list` — `screen.games[].name/size` metni yok; sadece satir kutusu + `neon` progress dolgusu.
- `sdl2_shell.rs:284` `draw_progress_screen` — secili oyunun `progress/status` metni yok.
- `sdl2_shell.rs:314` `draw_menu_overlay` ve `sdl2_shell.rs:358` `draw_virtual_keyboard`, `sdl2_shell.rs:405` `draw_folder_browser` — `ov.items`, `kb.layout`, `fb.path` etiketleri yerine `_label`/`_k` unused; sadece kutular.
- `manager-rs/manager-tvui/Cargo.toml` — `sdl2` var, `sdl2_ttf` yok; `theme.rs` renk paleti yuklu ama font yolu/olcegi TTF'e bagli degil.
- `accessibility.rs` `font_scale`/`footer_font_scale` degerleri hesaplaniyor fakat cizimde kullanilmiyor (DRIFT: kod ile son karar uyusmuyor — TASK-012k "done" isaretli oysa metin olcegi uygulanmiyor).
- WebUI SPA `?mode=tv` yaklasimi (`TASK-002q`) `superseded` (yon B native secildi); native shell metinsiz kaldigindan TVUI kullanilamaz durumda.

## Kapsam

- `sdl2_ttf` crate ekle (`manager-rs/Cargo.toml` workspace ve `manager-tvui/Cargo.toml` `[dependencies]` + `#[cfg(unix)]`/`#[cfg(windows)]` ayrimi yok — `sdl2_ttf` her ikisinde de derlenir, `freetype` Linux'ta sistem kutuphanesi).
- `theme.json` / `Theme` uzerinden font dosyasi cozumu (assets/fonts/ veya sistem font fallback) + `accessibility.font_scale` ile olcek.
- `render.rs` veya yeni `text.rs` modulu: `FontCache` (TTF init, boyut cache, `TextureCreator` ile `Solid`/`Blended` render -> `Texture`).
- `sdl2_shell.rs` tum `draw_*` fonksiyonlarina metin cizimi ekle: loading yuzdesi/hata mesaji, grid tile etiketi, game_list satir etiketi, progress yuzdesi, menu/klavye/browser etiketleri.
- `cargo check` ve `cargo check --target x86_64-pc-windows-gnu` dogrulamasi (AGENTS.md Cross-Platform).

## Kabul Kriterleri

- [x] `cargo check` geciyor (Linux; `manager-tvui` 1 pre-existing warning `state.rs:405` haric temiz).
- [x] Windows cross (`cargo check --target x86_64-pc-windows-gnu`) gecti (2026-09-16, `j1`: `CARGO_BUILD_JOBS=1 CMAKE_BUILD_PARALLEL_LEVEL=1`; paralelde Termux OOM signal 9 yedi). `fontdue`/`ab_glyph` pure-Rust, cross riski yok.
- [x] `draw_grid` platform isimleri `draw_text_centered` ile ciziliyor (kod bagli; headless ortamda gorsel dogrulama yapilamadi).
- [x] `draw_game_list` oyun isimleri/boyutlari ve progress yuzdesi metin olarak ciziliyor (kod bagli; gorsel dogrulama yapilamadi).
- [x] `draw_loading`/`draw_progress_screen` pct + hata mesaji metni (kod bagli; gorsel dogrulama yapilamadi).
- [x] `draw_menu_overlay`/`draw_virtual_keyboard`/`draw_folder_browser` etiketleri TTF ile ciziliyor; `font_scale` (`screen.a11y.font_scale()`, 0.5–3.0 clamp) metin boyutuna yansiyor.
- [x] `cargo test -p manager-tvui` 92/92 yesil (41 hedefi asildi); metin yardimcilari icin 2 unit test (`font_scale_clamping`, `font_scale_applies_proportionally` + `scaled_size` helper).

## Notlar

- `2026-09-03` DECISION: TVUI eksigi TTF ertelemesi olarak belgelendi | REASON: sdl2_shell metinsiz — kullanilamaz | SUPERSEDES: none
- `2026-09-16` DECISION: `sdl2_ttf` yerine `fontdue`+`ab_glyph` (pure-Rust rasterize → SDL texture) | REASON: `sdl2_ttf 0.25` vs `sdl2 0.37` sdl2-sys link catismasi | SUPERSEDES: spec'teki `sdl2_ttf` maddesi
- `2026-09-16` NOTE: `scaled_size(base,scale)` helper eklendi (8–64 clamp), `draw_text*` ikisi de kullaniyor; `state.rs` `visible_games:15` + 6-sutun grid navigasyon parity bu gap'in calisma agacinda geldi.
- Ilgili kararlar: TASK-012g/h/i/j/k "done" isaretli olmasina ragmen metin kismi eksik — bu gap DRIFT olarak isaretlendi.
