# TASK-002-gap-13 — Support ZIP Redaction (saf-Rust modda eksik, P0 güvenlik)

- **id:** TASK-002-gap-13
- **title:** /api/support secret redaction in pure-Rust (catalog-off) mode
- **status:** todo
- **priority:** P0
- **created:** 2026-08-15
- **environment:** both
- **tags:** security, support, redaction
- **parent:** TASK-002

## Karar (2026-08-15)

`/api/support` **response contract değişmez** (zip attachment döner). Ancak saf-Rust modda
(catalog yok) zip içine konan destek dosyalarındaki **gizli alanlar redakte edilmelidir** —
Python `test_support_zip.py` P0 güvenlik testinin parity'si.

> BELİRSİZ: redaksiyon mantığı saf-Rust'a nasıl port edilir? Python `utils/security.py`
> redaction fonksiyonu Rust'a mı port edilir, yoksa destek zip'i üretilmeden önce
> `manager-core` içinde merkezi bir `redact_secrets` geçişi mi eklenir? Kullanıcı onayı gerekir.

## Python Referans Davranışı

- `ports/RGSX/tests/test_support_zip.py` — Support ZIP içine yazılan log/settings dosyalarındaki
  şifre/API key gibi gizli alanların redakte edildiğini assert eder (P0).
- Redaksiyon kaynağı: `ports/RGSX/utils/security.py` (secret masking) + `rgsx_web/handlers.py`
  support handler'ı.

## Rust Mevcut Durum (❌)

- `manager-http/src/api.rs:790-809` `support()` — `catalog` varsa Python'a proxy (zip); yoksa
  **boş placeholder zip** döndürür (`:797-809`), hiçbir redaksiyon yapılmaz.
- `manager-http/src/api.rs:804-813` boş `Vec` ile zip üretilir; gizli alan filtresi YOK.
- `manager-http/tests/contract.rs:1433` yalnız binary proxy kontrolü yapar; **redaksiyon assert
  EDİLMEMİŞ**.

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-rs/manager-http/src/api.rs:790-813` — catalog-off dalında redaksiyon
- `manager-rs/manager-core/src/` — (olası) merkezi `redact_secrets` modülü — BELİRSİZ
- `manager-rs/manager-http/tests/contract.rs` — redaksiyon assert'i EKLENMELİ

## Bağımlılık

- Yok (bağımsız). `utils/security.py` portu gerekebilir ama mevcut endpoint'i bloklamaz.

## Doğrulama

- Saf-Rust modda üretilen support zip'inde şifre/API key görünmez (Python `test_support_zip.py`
  parity'si Rust testiyle sabitlenir).
- Mevcut proxy (catalog var) davranışı değişmez.
