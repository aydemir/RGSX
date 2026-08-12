# TASK-002k-2 — Faz 10c/3/2: Katalog route'ları Rust'e

- **id:** TASK-002k-2
- **title:** `platforms`/`search`/`games`/`image`/`translations` handler'larını Rust'e taşı
- **status:** done
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, katalog
- **parent:** TASK-002k

## Açıklama

Okuma-yönlü katalog route'ları (en düşük risk): Rust `manager-http` içinde gerçek mantık.
- `platforms`: `config.platform_dicts` (Rust'e taşınacak config state).
- `search`: `controls/search.py::search_games` mantığı (cache + kaynak sorgu).
- `games/{platform}`: `rgsx_web/cache.py::get_cached_games` mantığı.
- `image/{platform}`: box-art proxy (mevcut Python `handlers.py::_api_image`).
- `translations`: `language.py` çeviri paketi.

Python karşılıklarını `FAZ10C3_CONTRACT_MAP.md` (TASK-002k-1) referans alarak birebir taklit et.
Her route için `manager-http/tests/contract.rs`'a assertion ekle.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` (handler'lar doldurulur)
- `manager-rs/manager-core/src/` (gerekirse config/platform state)
- `manager-rs/manager-http/tests/contract.rs` (yeni assertion'lar)

## Doğrulama

- `cargo test -p manager-http` yeşil; Python `tests/test_api_contract.py` katalog route'ları yeşil.
- `RGSX_RUST_WEBUI` kapalı → Python davranışı değişmez.

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
- 2026-08-12 — **TAMAMLANDI (proxy stratejisi):** Onaylanan strangler/proxy yaklaşımıyla
  uygulandı. `manager-http/src/catalog.rs`: `CatalogSource` trait + `PythonCatalog` (reqwest,
  rustls-tls; `RGSX_PYTHON_MANAGER_URL`). `platforms`/`search`/`games`/`translations`/`image`
  handler'ları `state.catalog` varsa Python'a proxy'ler, yoksa placeholder (geriye uyumlu).
  `AppState.catalog` alanı + `main.rs` bağlama eklendi. `cargo test -p manager-http`: 74/74
  yeşil (6 yeni proxy testi). Native Rust logic portu ileride ayrı alt faz (bkz. FAZ10C3_CONTRACT_MAP.md).
