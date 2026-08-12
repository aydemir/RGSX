# TASK-002k-1 — Faz 10c/3/1: Keşif + sözleşme envanteri

- **id:** TASK-002k-1
- **title:** Rust placeholder route'larını Python karşılıklarına birebir eşle; boşluk matrisi üret
- **status:** done
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, keşif, contract
- **parent:** TASK-002k

## Açıklama

Rust `manager-http` placeholder handler'larını (bkz. TASK-002k) tek tek Python karşılıklarına
(map et: `rgsx_web/handlers*.py`, `controls/search.py`, `controls/handlers.py`, `rgsx_settings.py`,
`utils.py`, `network/queue.py`, `language.py`). Her route için: beklenen istek gövdesi/query, başarı
ve hata yanıtı şekli, SSE yayını, bağımlı config alanları. Çıktı:
`docs/roadmap/FAZ10C3_CONTRACT_MAP.md` (route × Python-modül × fonksiyon × contract-test).

## Kapsam / Dosyalar

- `docs/roadmap/FAZ10C3_CONTRACT_MAP.md` (YENİ)
- Referans: `manager-rs/manager-http/src/api.rs`, `manager-rs/manager-http/tests/contract.rs`,
  `tests/test_api_contract.py`

## Doğrulama

- Matris, 68 Rust + 364 Python contract testine referans verir; her placeholder route en az bir
  Python kaynağıyla eşlenir (eşlenemeyen → "DRIFT" olarak işaretlenir).

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
- 2026-08-12 — **TAMAMLANDI:** `docs/roadmap/FAZ10C3_CONTRACT_MAP.md` üretildi. Tüm Rust
  placeholder route'ları Python `ManagerHandler`/`RGSXHandler` karşılıklarına eşlendi; DRIFT
  alanları (system_info yanıt şekli, _api_update_cache/_serve_static gövdeleri) işaretlendi;
  alt görev↔route ataması yapıldı.
