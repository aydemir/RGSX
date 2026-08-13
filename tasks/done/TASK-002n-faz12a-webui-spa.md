# TASK-002n — Faz 12a: WebUI native SPA (Vue 3 + tower-http)

> Bağımlı: ROADMAP_FAZ12_RUST_WEBUI_TVUI.md (onaylandı). En düşük riskli faz.
> Hedef: tarayıcıda çalışan, refresh gerektirmeyen, SSE ile canlı ilerleyen
> Vue 3 SPA'yı Rust `manager-http` üzerinden sunmak.

## Kapsam
1. `webui/` — Vite + Vue 3 scaffold (canlı download progress barları SSE ile).
2. `manager-http`: `tower-http::ServeDir` ile `/static/*` servisi; `/` ve SPA
   route'ları (`/settings`,`/downloads`,...) hydrate edilmiş `index.html` döndürür
   (mevcut contract testleri korunur: `test_static_*`, `test_spa_*`).
3. SSE `/api/events` (zaten var, TASK-002m) Vue tarafından tüketilir.
4. `RGSX_WEBUI_DIR` ile build edilmiş `webui/dist` sunulur.

## Doğrulama
- `cargo test -p manager-http` → contract testleri yeşil (static + spa fallback).
- `cd webui && npm install && npm run build` → `dist/` üretilir.
- `RGSX_WEBUI_DIR=webui/dist RGSX_RUST_WEBUI=1 cargo run -p manager-bin` →
  tarayıcıda canlı barlar.
