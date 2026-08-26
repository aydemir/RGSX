# TASK-002-gap-15 — Rust-Only Endpoint Test Kapsamı (languages / scan / es-input + language auto-detect)

- **id:** TASK-002-gap-15
- **title:** Rust-only endpoint + language auto-detect test coverage
- **status:** done
- **priority:** P2
- **created:** 2026-08-15
- **environment:** both
- **tags:** test, api, coverage
- **parent:** TASK-002

## Karar (2026-08-15)

Bu TASK **parite boşluğu DEĞİL** — Python'da karşılığı olmayan Rust-only uç noktaların test
kapsamı açığıdır. `/api/languages`, `/api/scan`, `/api/es-input` uç noktaları ve dil
auto-detect mantığı `contract.rs`'te test EDİLMEMİŞ.

## Python Referans Davranışı

- `ports/RGSX/tests/test_language.py` — ilk açılışta dil auto-detect.
- Rust-only uç noktaların Python karşılığı yok (parite değil, kapsama açığı).

## Rust Mevcut Durum (⚠️ test yok)

- `manager-http/src/lib.rs:41` `GET /api/languages` — test EDİLMEMİŞ.
- `manager-http/src/lib.rs:50` `GET /api/scan` — test EDİLMEMİŞ.
- `manager-http/src/lib.rs:52` `GET /api/es-input` — test EDİLMEMİŞ.
- Dil auto-detect mantığı Rust'ta test EDİLMEMİŞ (mekanizma Python TVUI'den farklı).

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-rs/manager-http/tests/contract.rs` — 3 uç nokta + dil auto-detect testi EKLENMELİ
- `manager-rs/manager-http/src/catalog_bootstrap.rs` / `es_input.rs` — auto-detect mantığı (test hedefi)

## Bağımlılık

- Yok (bağımsız). İlgili uç noktalar zaten mevcut.

## Doğrulama

- `contract.rs` içinde `/api/languages`, `/api/scan`, `/api/es-input` için response sözleşme testi.
- Dil auto-detect regresyon testi (ilk açılış varsayılanı).

## 2026-08-18 UYGULANDI — contract testleri + tasarım notu

`manager-rs/manager-http/tests/contract.rs`'e 4 test eklendi (hepsi geçti, 113/113):
- `test_languages_shape` — `/api/languages` fallback `["en","tr"]` döner.
- `test_scan_shape` — `/api/scan` success+root/platforms/total_bytes/total_files/disk şekli.
- `test_es_input_shape` — `/api/es-input` her durumda boolean `found` (RetroBat/Batocera `es_input.cfg` okuması `es_input.rs` ile zaten mevcut).
- `test_language_autodetect_standalone_default` — `/api/translations` boş state'de somut (boş olmayan) bir dil döndürür.

**Tasarım kararı (kullanıcı yönlendirmesi):** RetroBat/Batocera'da kullanıcı ÖNCE kendi dil+gamepad
ayarını yapar, RGSX sonra kurulur. Bu yüzden dil auto-detect **tarayıcı/OS algısı DEĞİL**, sunucu
tarafı varsayılanıdır:
- Katalog (Python bridge) modunda dil `RGSX_LANGUAGE` env'inden okunur (kurulum katalogdan ayarlar).
- Bağımsız/embedded modda gömülü varsayılan ("tr") döner.
- Gamepad/ES-input `manager-http/src/es_input.rs` zaten host `es_input.cfg`'yi okuyor
  (`/api/es-input` → `load_best()`).

Bu "host conf'dan oku, kullanıcı seçimini ezme" tasarımı **korundu** (değiştirilmedi). gap-15 yalnızca
eksik test kapsamını kapattı; davranış değişikliği yok.
