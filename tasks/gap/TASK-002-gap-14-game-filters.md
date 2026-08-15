# TASK-002-gap-14 — Game Filters Saf Mantık + Test (Rust'ta test edilmemiş)

- **id:** TASK-002-gap-14
- **title:** game_filters pure logic port + Rust test coverage
- **status:** todo
- **priority:** P2
- **created:** 2026-08-15
- **environment:** both
- **tags:** filters, test, parity
- **parent:** TASK-002

## Karar (2026-08-15)

`/api/save_filters` **request contract değişmez.** Python `game_filters.py` içindeki saf filtreleme
iş mantığı Rust'a port edilir ve `contract.rs` ile round-trip test edilir.

> BELİRSİZ: filtreleme mantığı `manager-core` mı yoksa ayrı bir `manager-scan` filtresi mi
> olur? Python `game_filters.py` hangi modüle karşılık geliyor — uygulamaya başlamadan doğrulanacak.

## Python Referans Davranışı

- `ports/RGSX/tests/test_game_filters.py` — `GameFilters` saf iş mantığını (include/exclude,
  platform filtresi, metin eşleme) test eder.
- `ports/RGSX/game_filters.py` — filtre uygulama mantığı.

## Rust Mevcut Durum (❌ test yok)

- `/api/save_filters` yalnız proxy: `manager-http/tests/contract.rs:1344`.
- Filtreleme iş mantığı Rust'ta **test EDİLMEMİŞ**; saf-Rust modda filtre uygulaması parity'si yok.

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-rs/manager-core/` veya `manager-scan/` — filtre mantığı (Python `game_filters.py` karşılığı)
- `manager-rs/manager-http/src/api.rs` — `/api/save_filters` handler (veya doğrudan settings persist)
- `manager-rs/manager-http/tests/contract.rs` — `test_game_filters` parity testi EKLENMELİ

## Bağımlılık

- Yok (bağımsız). `TASK-006-native-settings-webui` ile örtüşebilir — BELİRSİZ, çakışma kontrolü gerekir.

## Doğrulama

- Python `test_game_filters.py` senaryolarının Rust eşdeğeri yeşil.
- `/api/save_filters` round-trip (kaydet → oku) Rust'ta test edilir.
