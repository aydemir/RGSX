# TASK-002e — manager-http: WebUI statik servisi (/ + /static/*)

- **id:** TASK-002e
- **title:** Rust manager WebUI statik servisi — index.html + /static/*
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **tags:** rust, axum, static, webui
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10 (Rust kısmi refaktör).
  Canlı test sırasında ortaya çıktı: Rust manager-bin `/` ve `/static/*`'ı servis etmiyor
  (placeholder HTML + 404), WebUI yüklenemiyor.

## Açıklama

Python `ManagerHandler` (`rgsx_web/handlers.py`) index sayfasını `_get_index_html()` ile üretir ve
`/static/*` dosyalarını servis eder. Rust `manager-http` yalnızca `/api/*` contract'ı taşıyor; `/`
minimal placeholder, `/static/*` 404 döner. Amaç: Rust tarafında da WebUI'yi servis edebilmek.

**Tek kaynak kararı:** Index HTML template'i `ports/RGSX/static/index.html`'e taşınır; Python
`_get_index_html()` bu dosyadan okur (placeholder değişimleri korunur), Rust `/` de aynı dosyayı
okur. Çift bakım yok.

## Kapsam / Dosyalar

- `ports/RGSX/static/index.html` — Python `_get_index_html` template'inin birebir kopyası
  (`__CSS_VERSION__`, `__JS_VERSION__`, `{version}` placeholder'ları korunur)
- `ports/RGSX/rgsx_web/handlers_ui.py` — `_get_index_html()` dosyadan okur; dosya yoksa eski
  inline fallback korunur
- `manager-rs/manager-http/src/state.rs` — `AppState.static_root: Option<PathBuf>`
- `manager-rs/manager-http/src/api.rs` — `/` index servisi + `/static/*` dosya servisi (yol
  traversal koruması, mime tespiti)
- `manager-rs/manager-http/src/lib.rs` — route ekleme (`/static/*`)
- `manager-rs/manager-bin/src/main.rs` — `static_root`'u script yanındaki `static/` klasörüne işaret et
- `manager-rs/manager-http/tests/contract.rs` — `/` + `/static/*` testleri

## Doğrulama

- `cargo test --workspace` geçer
- Python suite baseline değişmez (index içerik assert'i yok; sadece `text/html` header)
- Canlı: manager-bin başlat → `/` gerçek WebUI HTML, `/static/js/app.js` 200, tarayıcıda WebUI yüklenir

---

## İlerleme

- 2026-08-12 — TASK-002e tanımlandı (canlı test bulgusu: WebUI servis edilmiyor)
- 2026-08-12 — Kapsam tamamlandı: `static/index.html` (Python `_get_index_html` birebir),
  `AppState.static_root`, `/` + `/static/*` servis, path traversal koruması, mime tespiti,
  `{version}`/`__CSS_VERSION__`/`__JS_VERSION__` hydration, SPA fallback (`/settings`,
  `/downloads`, `/history`, `/platform/*` → index; Python `handlers.py:111` birebir).
  `cargo test --workspace` geçer (manager-http 63 test); canlı smoke: `/`, `/settings`,
  `/platform/NES` gerçek WebUI HTML (title `RGSX Web Interface`, placeholder kalmadı).
