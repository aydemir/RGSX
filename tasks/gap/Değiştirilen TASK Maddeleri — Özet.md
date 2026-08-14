# TASK-002-gap-1 — Değiştirilen Maddeler Özeti

**Karar:** KEEP_CONTRACT — `retry_count`/`max_retries` `/api/download` request contract'ına eklenmez.
**Tarih:** 2026-08-14
**Dal:** custom

## Değiştirilen TASK Maddeleri

- **KARAR (KEEP_CONTRACT)** bölümü eklendi:
  - `/api/download` request contract değişmez.
  - `retry_count` ve `max_retries` request parametresi DEĞİLDİR; Rust retry motorunun internal
    state/config değeridir.
  - Sonuçlar yalnızca `/api/history` **response/history contract** tarafında Python parity'siyle
    taşınır (additive — mevcut 105 contract testi kırılmaz).

- **Eski TASK:38 düzeltildi** (yanlış: `manager-bin/src/ — /api/download sözleşmesine
  retry_count/max_retries alanı`):
  - Yerine `manager-http/src/api.rs` ortak retry katmanı + `manager-core` retry modülü yazıldı.
  - "contract" kelimesi artık açıkça "request contract" / "response/history contract" olarak niteleniyor.

- **Eski TASK:37 düzeltildi** (yanlış: `manager-torrent/src/lib.rs` bağımsız retry motoru):
  - Retry'ın **her iki download yolunu** (`api.rs:421-540` torrent bridge + `api.rs:1101-1197`
    native DDL/HTTP) kapsadığı ve ortak entegrasyon noktasının `api.rs` spawn/finalize akışları
    olduğu açıkça tarif edildi.
  - `manager-torrent/src/lib.rs` içine torrent'e özel bağımsız retry motoru eklenmez.

- **Python Parity — Retry Akışı** diyagramı eklendi:
  `download başarısız → retry_count oku → max_retries config (default 3) → classify_error →
  Transient + (retry_count < max_retries) → retry_count++ → backoff → RETRY_SCHEDULED →
  retry_at → yeniden download`. Permanent hata veya budget exhausted → retry YAPILMAZ, finalize
  korunur.

- **State machine mevcut, motor eksik** ayrımı eklendi: `manager-core/src/state.rs` içindeki
  `RetryScheduled`/`FailedTransient`/`RetryTriggered`/`RetryExhausted` state/transition kuralları
  yeniden icat edilmez; retry motoru bunları **kullanır**.

- **History Parity** bölümü eklendi: `finalize_download_in_state` parity için history entry'ye
  `entity_state`/`retry_count`/`max_retries`/`retry_at` yazmalı. Bu request contract değişikliği
  DEĞİLDİR; `Python /api/history = Rust /api/history` hedefi additive olarak ele alınır.

- **Kapsam Dışı** bölümü eklendi (6 madde): request schema değişikliği, yeni client retry
  parametresi, Python worker'a retry gönderme, `manager-torrent`'e bağımsız engine, yeni
  endpoint, contract test değişimi — hepsi dışlandı.

## Doğrulama (9 soru)

1. `/api/download` request contract değişiyor mu? → HAYIR
2. `retry_count` nerede tutulur? → Rust internal retry state/history
3. `max_retries` nereden geliyor? → Rust config/default, default=3
4. `/api/history` retry alanlarını taşıyor mu? → Evet; Rust Python parity'sini sağlamalı
5. Retry hangi download yollarını kapsıyor? → Torrent bridge + Native DDL/HTTP
6. Retry motorunun ortak entegrasyon noktası neresi? → `manager-http/src/api.rs` download/finalize akışları
7. `manager-torrent/src/lib.rs` içine bağımsız retry motoru ekleniyor mu? → HAYIR
8. Mevcut state machine yeniden tasarlanıyor mu? → HAYIR
9. `/api/download` contract testleri değişecek mi? → HAYIR

## Implementation'a Hazır mı?

EVET. Değiştirilecek dosyalar:
- `manager-rs/manager-core/src/retry.rs` (yeni): `classify_error`, `retry_backoff_seconds`,
  sabitler, unit test.
- `manager-rs/manager-http/src/state.rs`: `StateData`'ya `retries: HashMap<url,u32>` +
  `retry_in_flight: HashSet<String>`.
- `manager-rs/manager-http/src/api.rs`: iki spawn gövdesini saran retry döngüsü +
  `finalize_download_in_state` parity yazımı (entity_state/retry_count/max_retries/retry_at) +
  cancel/shutdown kontrolü.
