# TASK-002c — Python arka plan köprüsü + manager-bin entegrasyonu

- **id:** TASK-002c
- **title:** qbittorrent_backend subprocess köprüsü + binary entegrasyonu
- **status:** todo
- **priority:** P2
- **created:** 2026-08-12
- **tags:** rust, bridge, subprocess, tokio, manager-bin
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10 "Ara mimari": Rust manager
  binary, `qbittorrent_backend.py`'yi subprocess çağırır (JSON-RPC / local HTTP köprüsü).

## Açıklama

Rust manager binary (manager-bin), mevcut `qbittorrent_backend.py`'yi **subprocess** olarak
başlatır ve local JSON-RPC / HTTP köprüsüyle konuşur. Downloader mantığı Python'da kalırken
Windows tarafı kademeli Rust'a geçer; Linux/Batocera Python yolunu korur.

Ayrıca manager-core state machine + manager-http (TASK-002b) + bridge entegrasyonu tek
binary'de birleştirilir: sağlık döngüsü (watchdog logic), manager durum yayını, `/api/*`
eylemlerinin libsiye bağlanması.

## Kapsam / Dosyalar

- `manager-rs/manager-bridge/src/lib.rs` — subprocess spawn + JSON-RPC/local HTTP protokolü
- `manager-rs/manager-bin/src/main.rs` — tokio runtime: core state + http + bridge birleşimi
- Python tarafı: bridge protokolüne uygun minimal uç (mevkii Python'da kalır)

## Doğrulama

- `cargo test` (workspace) + `cargo check --workspace` geçer
- Köprü: subprocess'teki Python karşıtıyla echo/RPC round-trip testi
- Canlı: `manager-bin` ayağa kalkar, `/api/*` eylemleri Python'da gerçek iş yürütür,
  SSE event'leri yayınlanır; `qbittorrent_backend` sağlıklı
- Python suite baseline değişmez

---

## İlerleme

- 2026-08-12 — Alt-görev tanımlandı (Windows kapsamına alındı; Linux/ARM yokluğu 10b'yi engeller)