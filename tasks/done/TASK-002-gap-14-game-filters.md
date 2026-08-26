# TASK-002-gap-14 — Game Filters Saf Mantık + Test (Rust'ta test edilmemiş)

- **id:** TASK-002-gap-14
- **title:** game_filters pure logic port + Rust test coverage
- **status:** done
- **priority:** P2
- **created:** 2026-08-15
- **updated:** 2026-08-18
- **environment:** both
- **tags:** filters, test, parity
- **parent:** TASK-002

## Karar (çözüldü 2026-08-18)

`/api/save_filters` **request contract değişmedi.** Python `game_filters.py` içindeki saf
filtreleme iş mantığı Rust'a port edildi ve `contract.rs` + birim testleri ile round-trip
test edildi.

> BELİRSİZ kararlar:
> - **Modül = `manager-core`** — saf mantık, I/O yok; `Settings.game_filters` zaten burada
>   (`settings.rs`), genişçe import edilir. `manager-scan` yerine `manager-core` seçildi.
> - **TASK-006 çakışması YOK** — `tasks/done/TASK-006-native-settings-webui.md` zaten **done**
>   ve filtre mantığına hiç dokunmuyor (grep: filtre kelimesi geçmiyor).

## Python Referans Davranışı

- `ports/RGSX/game_filters.py` — `GameFilters` sınıfı (`REGIONS`, `region_filters`,
  `hide_non_release`, `one_rom_per_game`, `hide_downloaded`, `regex_mode`, `region_priority`),
  `get_game_regions`, `is_non_release_game`, `get_base_game_name`, `apply_filters`,
  `_apply_one_rom_per_game`, `get_region_priority`.
- Python `tests/test_game_filters.py` **mevcut değil** (doc eski referans) — parity, modül
  davranışına göre yazılan Rust testleri ile doğrulandı.

## Rust Uygulanan (2026-08-18)

- `manager-rs/manager-core/src/game_filters.rs` (yeni modül, `lib.rs`'te `pub mod game_filters`):
  - `GameFilters` struct (serde Serialize/Deserialize) + `new()`, `load_from_dict()`,
    `to_dict()`, `is_active()`, `reset()`, `apply_filters()`.
  - Saf yardımcılar (Python `@staticmethod` parity'si): `get_game_regions`, `is_non_release_game`,
    `get_base_game_name` (bölge çıkarımı, beta/demo/proto/... regex'i, taban isim soyma).
  - `apply_filters` include/exclude bölge, `hide_non_release`, `one_rom_per_game` (bölge önceliği
    ile tek ROM seçimi) mantığını uygular; `hide_downloaded` enjekte edilen `is_downloaded`
    closure ile (pure-test, Python `is_game_downloaded` karşılığı).
  - `regex` crate (zaten bağımlılıkta) + `std::sync::OnceLock` ile derlenen regex'ler; ek bağımlılık yok.
- `manager-rs/manager-http/tests/contract.rs` — `test_game_filters` parity testi eklendi
  (bölge exclude, one-rom-per-game öncelik, non-release gizleme).

## Doğrulama

- ✅ `manager-core` birim testleri (9): `regions_from_name`, `non_release_detection`,
  `base_game_name_strips_regions_and_ext`, `apply_filters_region_exclude`,
  `apply_filters_hide_non_release`, `apply_filters_one_rom_per_game_...`, `apply_filters_inactive_returns_all`,
  `load_and_to_dict_roundtrip`, `load_from_dict_defaults_missing_regions_to_include`.
- ✅ `contract.rs` `test_game_filters` geçti.
- ✅ Tam suite: 114 contract + 20 lib test (manager-http) yeşil, regresyon yok.

## Not — uygulama noktası (kapsam dışı)

Saf mantık portlandı ve test edildi. Filtrelerin **canlı oyun listesine uygulanması**
(native modda `catalog` proxy yokken) ayrı bir adımdır; Python `catalog` proxy mevcutken
filtreleme zaten Python tarafında yapılıyor. Bu görev saf-mantık + parity testini kapsar.
