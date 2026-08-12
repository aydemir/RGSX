# TASK-002k-4 — Faz 10c/3/4: Destek/queue yönetimi route'ları Rust'e

- **id:** TASK-002k-4
- **title:** `support`/`queue_*`/`clear_history`/`restart`/`pause`/`resume`/`cancel`/`shutdown` Rust'e
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, queue, destek
- **parent:** TASK-002k

## Açıklama

Yönetim/eylem route'ları:
- `support`: `utils.generate_support_zip` → Rust'te destek ZIP üretimi (log/history toplama).
- `queue`/`queue_clear`/`queue_remove`: `manager-core` queue state + SSE yayını (zaten `queue` var).
- `clear_history`: history.json temizleme.
- `restart`: manager yeniden başlatma sinyali.
- `pause`/`resume`: kuyruk duraklat/devam (aktif slot sayısı).
- `cancel`/`shutdown`: gerçek iptal/sunucu kapanışı (şu an placeholder → fonksiyonel yapılır).

`cancel`, Rust torrent devrinde (Faz 10c/2) hâlâ placeholder; burada gerçek iptal `TorrentBackend`
üzerinden bağlanır.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs`
- `manager-rs/manager-core/src/state.rs` (queue/history/pause state)
- `manager-rs/manager-http/tests/contract.rs`

## Doğrulama

- `cargo test -p manager-http` + Python contract testleri yeşil. `cancel` artık gerçek iptal doğrular.

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
