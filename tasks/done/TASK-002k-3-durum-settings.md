# TASK-002k-3 — Faz 10c/3/3: Durum/settings route'ları Rust'e

- **id:** TASK-002k-3
- **title:** `settings_get`/`settings_post`/`save_filters`/`system_info`/`browse_directories`/`game_status` Rust'e
- **status:** done
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, settings
- **parent:** TASK-002k

## Açıklama

Ayar + durum route'ları: `rgsx_settings.py` (okuma/yazma/persist) ve `config` state'i Rust'e
taşınır. `system_info` contract'ı **en kritik** — `tests/test_api_contract.py` birebir eşitlik
testi var; Yanıt şekli (versions/paths/flags) değişemez. `browse_directories` dosya tarayıcı,
`game_status` indirilen/aktif/hatalı durum özeti.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs`
- `manager-rs/manager-core/src/state.rs` (config/settings state genişletmesi)
- `manager-rs/manager-http/tests/contract.rs` (`system_info` dahil)

## Doğrulama

- `system_info` contract testi yeşil kalır (birebir eşitlik). `cargo test -p manager-http` yeşil.

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
