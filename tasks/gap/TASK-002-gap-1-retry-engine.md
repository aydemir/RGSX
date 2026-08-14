# TASK-002-gap-1 — Retry/Backoff Motoru (Rust'ta eksik)

- **id:** TASK-002-gap-1
- **title:** Retry/backoff engine (transient sınıflandırma + zamanlanmış retry)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** download, retry, state-machine
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `F1 → F2 → R0..R3`
- `ports/RGSX/network/queue.py`: `_finalize_download_result` (satır 468),
  `_schedule_download_retry` (satır 572), `_retry_backoff` (satır 448),
  `_max_retries` (satır 444), `_retry_in_flight` (satır 441)
- `ports/RGSX/network/download_state.py`: `classify_error`, `retry_backoff_seconds`,
  `DEFAULT_MAX_RETRIES` (3), `DEFAULT_BACKOFF_BASE_SEC` (5.0), `DEFAULT_BACKOFF_MAX_SEC` (300.0)
- `manager-rs/manager-core/src/state.rs`: `DownloadEvent::{TransientFailure, RetryTriggered,
  RetryExhausted, RetryScheduled}` ve transition tablosu MEVCUT (TASK-002a) — **ama motor yok**

## Açıklama

Python tarafında başarısız bir indirme `classify_error(message)` ile transient/kalıcı ayrılır.
Transient ve `retry_count < max_retries` ise state `RETRY_SCHEDULED`'a geçer ve
`_schedule_download_retry` bir backoff sonrası aynı URL'yi yeni task_id ile yeniden indirir.
`_retry_in_flight` set'i çift retry'yi engeller; `_app_shutting_down`/cancel kontrolü yapılır.
Rust `DownloadState` enum'unda `RetryScheduled`/`RetryTriggered` varyantları ve transition
kuralları tanımlı, ancak **gerçek retry'i tetikleyen, backoff hesaplayan ve slot bekleyen
motor yok**. Rust daemon (`manager-bin` / `manager-torrent`) bir indirme failed dediğinde
otomatik yeniden deneme yapmıyor.

## Kapsam / Dosyalar

- `manager-rs/manager-core/src/` — retry backoff hesabı (Rust karşılığı `retry_backoff_seconds`)
- `manager-rs/manager-torrent/src/lib.rs` — `download_torrent` içine retry döngüsü + `_retry_in_flight` dedup
- `manager-rs/manager-bin/src/` — `/api/download` sözleşmesine `retry_count`/`max_retries` alanı

## Doğrulama

- `classify_error` transient kuralları Rust'a birebir taşınır (test: ağ hatası transient, parse hatası kalıcı).
- Backoff = `min(max_wait, base * 2^(n-1))` formülü Rust'ta doğrulanır.
- Çift retry: aynı URL için eşzamanlı iki retry thread'i `_retry_in_flight` ile engellenir.
- `RetryExhausted` event'i `FailedPermanent`'a geçer (state.rs transition zaten tanımlı).
