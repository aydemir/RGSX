# TASK-002-gap-1 — Retry/Backoff Motoru (Rust'ta eksik)

- **id:** TASK-002-gap-1
- **title:** Retry/backoff engine (transient sınıflandırma + zamanlanmış retry)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** download, retry, state-machine
- **parent:** TASK-002

## KARAR (KEEP_CONTRACT)

`/api/download` **request contract değişmez.** `retry_count` ve `max_retries` request
parametresi DEĞİLDİR; bu alanlar Rust retry motorunun **internal state/config** değeridir.
Retry sonuçları yalnızca `/api/history` **response/history contract** tarafında Python
parity'siyle taşınır (additive — mevcut 105 contract testi kırılmaz).

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `F1 → F2 → R0..R3`
- `ports/RGSX/network/queue.py`: `_finalize_download_result` (satır 468),
  `_schedule_download_retry` (satır 572), `_retry_backoff` (satır 448),
  `_max_retries` (satır 444), `_retry_in_flight` (satır 441)
- `ports/RGSX/network/download_state.py`: `classify_error`, `retry_backoff_seconds`,
  `DEFAULT_MAX_RETRIES` (3), `DEFAULT_BACKOFF_BASE_SEC` (5.0), `DEFAULT_BACKOFF_MAX_SEC` (300.0)
- `ports/RGSX/rgsx_manager.py:347-351` — `_handle_download_worker` request'ten yalnızca
  `platform`/`game_index`/`game_name`/`url`/`mode` okur; **`retry_count`/`max_retries` YOK.**
- `manager-rs/manager-core/src/state.rs`: `DownloadEvent::{TransientFailure, RetryTriggered,
  RetryExhausted, RetryScheduled}` ve transition tablosu MEVCUT (TASK-002a) — **ama motor yok**
- `manager-rs/manager-http/src/api.rs:399` `download(... Json<Value>)` — request untyped,
  `body.get("url")`, `body.get("game_name")` okur; **`retry_count`/`max_retries` YOK.**
- `manager-rs/manager-http/src/api.rs:1044-1096` `finalize_download_in_state` — yalnızca
  `status`/`message`/`progress` yazar; `entity_state`/`retry_count`/`max_retries`/`retry_at` YAZMAZ.
- `manager-rs/manager-http/src/state.rs:22-41` `StateData` — `history: Vec<Value>`, retry için
  ayrı alan yok.
- `tests/test_api_contract.py:312,321,327` — `/api/download` POST'ları retry alanı içermez;
  grep'te retry assertion'ı sıfırdır.

## Açıklama

Python tarafında başarısız bir indirme `classify_error(message)` ile transient/kalıcı ayrılır.
Transient ve `retry_count < max_retries` ise state `RETRY_SCHEDULED`'a geçer ve
`_schedule_download_retry` bir backoff sonrası aynı URL'yi yeni task_id ile yeniden indirir.
`_retry_in_flight` set'i çift retry'yi engeller; `_app_shutting_down`/cancel kontrolü yapılır.
Rust `DownloadState` enum'unda `RetryScheduled`/`RetryTriggered` varyantları ve transition
kuralları tanımlı, ancak **gerçek retry'i tetikleyen, backoff hesaplayan ve slot bekleyen
motor yok**. Rust daemon (`manager-bin` / `manager-torrent`) bir indirme failed dediğinde
otomatik yeniden deneme yapmıyor.

### State machine mevcut, motor eksik

`manager-core/src/state.rs` içindeki `RetryScheduled` / `FailedTransient` / `RetryTriggered` /
`RetryExhausted` state'leri ve transition kuralları **yeniden icat edilmeyecek**. Eksik olan,
bu state'lere geçişleri tetikleyen, backoff hesaplayan ve yeniden indirmeyi başlatan **motor**'dur.
Retry motoru mevcut state machine'i **kullanır** (event'leri üretir), tanımlamaz.

## Python Parity — Retry Akışı (referans)

```
download başarısız
    ↓
retry_count oku (history entry'den, Python: queue.py:483)
    ↓
max_retries config'den al (Python: queue.py:444 _max_retries → config.DOWNLOAD_MAX_RETRIES, default 3)
    ↓
classify_error(message)  (Python: download_state.py:195)
    ↓
Transient + (retry_count < max_retries)  ?
    ├─ EVET → retry_count++
    ↓          backoff hesapla (retry_backoff_seconds; min(max_wait, base*2^(n-1)))
    ↓          RETRY_SCHEDULED  (entity_state="RETRY_SCHEDULED")
    ↓          retry_at = now + delay
    ↓          yeniden download (yeni task_id)
    ↓
    └─ HAYIR (Permanent hata VEYA budget exhausted) → retry YAPILMAZ
               mevcut failure/finalize davranışı korunur (FAILED_PERMANENT)
```

Retry motoru şu konuları kapsamalıdır:
- transient/permanent classification (`classify_error` birebir port)
- exponential backoff (`min(max_wait, base * 2^(n-1))`, base=5.0, max=300.0)
- retry limit (`max_retries`, default=3)
- duplicate/in-flight retry kontrolü (`_retry_in_flight` set'i parity'si)
- retry state/history güncellemesi (aşağıdaki History Parity bölümü)
- cancellation/shutdown kontrolü (`_app_shutting_down` / iptal edilmiş task_id parity'si)

## Kapsam / Dosyalar

### Ortak entegrasyon noktası: `manager-http/src/api.rs`

Kod incelemesinde kesinleşen mimari: **her iki download yolu da `manager-http/src/api.rs`
içindeki `tokio::spawn` akışına girer ve `finalize_download_in_state(...)` sonucuna bağlanır.**

- **Torrent bridge yolu:** `manager-http/src/api.rs:421-540` (bridge `download_torrent_progress`,
  `:528`) → `finalize_download_in_state` (`:533`, `:540`).
- **Native DDL/HTTP yolu:** `manager-http/src/api.rs:1101-1197` (`native_ddl_download`,
  `HttpDownloader::download_async`) → `finalize_download_in_state` (`:1185`, `:1197`).

Retry **torrent'e özgü değildir**; her iki yolu da kapsamalıdır. Bu nedenle retry motoru
`manager-http/src/api.rs` içindeki **iki download spawn/finalize akışını saran ortak katman**a
konulur (her iki dalın `Err` sonucunda retry döngüsü). Retry davranışı burada her iki yola da
uygulanır.

- `manager-rs/manager-core/src/` — retry backoff hesabı (Rust karşılığı `retry_backoff_seconds`)
  + `classify_error` portu (yeni `retry.rs` modülü önerilir). `DEFAULT_MAX_RETRIES=3`,
  `DEFAULT_BACKOFF_BASE_SEC=5.0`, `DEFAULT_BACKOFF_MAX_SEC=300.0` sabitleri korunur.
- `manager-rs/manager-http/src/api.rs` — her iki download spawn akışını saran retry döngüsü;
  `retry_count` internal state olarak (örn. `StateData` içi `HashMap<url, u32>` veya history
  entry üzerinde) tutulur; `max_retries` config/default (default=3); in-flight dedup için
  `HashSet<url>` (AppState veya StateData); cancel/shutdown kontrolü.

> NOT: `manager-torrent/src/lib.rs` içine **torrent'e özel bağımsız retry motoru eklenmez.**
> Retry, `api.rs` ortak katmanında her iki yolu da kapsar.

## History Parity (Rust ≡ Python /api/history)

Mevcut Rust `finalize_download_in_state()` (`manager-http/src/api.rs:1044-1096`) yalnızca
`status`/`message`/`progress` yazar. Python'da retry sırasında history entry'ye şunlar yazılır
(`download_state.py:230-248` `apply_to_history_entry`, `queue.py:537-543`):

- `entity_state`
- `retry_count`
- `max_retries`
- `retry_at`

Rust native retry implementasyonu **bu parity'yi sağlamalıdır**: retry durumunda (ve nihai
sonuçta) history entry'ye bu dört alan yazılır. `history` handler
(`manager-http/src/api.rs:219-237`) entry'leri `Value` olarak döndürdüğü için bu alanlar
additive olarak `/api/history` **response/history contract**'ına yansır.

**Bu, `/api/download` request contract değişikliği DEĞİLDİR.** Amaç:

```
Python /api/history davranışı  =  Rust /api/history davranışı
```

Bu değişiklik **additive response/history parity** olarak ele alınır; mevcut `/api/download`
contract testleri (`tests/test_api_contract.py`) retry parametresi eklenmediği için DEĞİŞMEZ.

## Kapsam Dışı (gereksiz büyütme engellendi)

Aşağıdakiler TASK kapsamına **girmez**:
- `/api/download` request schema değişikliği
- yeni client retry parametreleri
- Python worker'a retry parametresi gönderme
- `manager-torrent/src/lib.rs` içine bağımsız retry engine
- yeni bir API endpoint
- mevcut contract testlerinin gereksiz değiştirilmesi

## Doğrulama

- `classify_error` transient kuralları Rust'a birebir taşınır (test: ağ hatası transient, parse hatası kalıcı).
- Backoff = `min(max_wait, base * 2^(n-1))` formülü Rust'ta doğrulanır (manager-core retry modülü unit test).
- Çift retry: aynı URL için eşzamanlı iki retry `_retry_in_flight` (HashSet) ile engellenir.
- `RetryExhausted` event'i `FailedPermanent`'a geçer (state.rs transition zaten tanımlı).
- History parity: retry sırasında `entity_state`/`retry_count`/`max_retries`/`retry_at` history
  entry'ye yazılır; `/api/history` response'u Python ile uyumlu (additive).
- `tests/test_api_contract.py` 105 contract testi **değişmeden** yeşil kalır (request contract değişmedi).
