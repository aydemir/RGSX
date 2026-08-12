# TASK-002f — Faz 10b: librqbit embedded torrent engine (askıdaki iş)

- **id:** TASK-002f
- **title:** embedded torrent: qbittorrent_backend yerine librqbit engine (askıdaki iş)
- **status:** in-progress
- **priority:** P2
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, librqbit, torrent, manager-torrent, faz-10b
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10b (`librqbit` adayı).
- **Blocker çözüldü:** 10b "Linux/ARM test imkânı şart olduğundan başlanmadı" diye askıdaydı;
  bu sandbox **aarch64 Linux** olduğundan şart sağlandı.

## Açıklama

Mevcut `qbittorrent_backend.py` (1853 satır) embedded qBittorrent binary'sini yönetip WebUI
REST üzerinden konuşur. Faz 10b: bu backend'in yerine Rust içinde **librqbit** (rqbit motoru,
max_stable 8.1.1) embedded torrent engine olarak ikame edilir. Platform-bağımsız olduğundan
tek Rust paketi Windows+Linux/Batocera'yı kapsar; Batocera Python yolu korunur.

**Korunması gereken contract'lar (kırılamaz):**
- stdio JSON-RPC köprüsü metodları (`_BRIDGE_METHODS`: ping/status/is_available/ensure_running/
  get_webui_url/get_password_status/change_webui_password/shutdown) — manager-bridge bunu konuşur.
- İndirme akışı `download_torrent_via_qbittorrent`: tag/file-selection/seed takibi.
- Backend state makinesi (STOPPED/STARTING/PORT_RESOLVING/WEBUI_AUTH_WAIT/RUNNING/...).

**Yaklaşım (kademeli, 3 adım):**
1. **Spike/fizibilite:** `manager-torrent` crate + librqbit, aarch64'te derle; gerçek public
   torrent ile indirme/seed doğrula.
2. **Bridge uyumlu torrent serve:** librqbit üzerinde mevcut JSON-RPC metodlarını sergileyen
   `--bridge` uyumlu uç.
3. **Entegrasyon:** manager-bin'de `RGSX_TORRENT_ENGINE=librqbit` seçimi; Python fallback korunur.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/` (YENİ crate) — librqbit wrapper + engine
- `manager-rs/Cargo.toml` — workspace member + librqbit bağımlılığı
- `manager-rs/manager-bin/src/main.rs` — engine seçimi + köprü
- Test: `manager-rs/manager-torrent/tests/` — birim + canlı torrent testleri

## Doğrulama

- `cargo test --workspace` + `cargo check --workspace` geçer
- Canlı (aarch64): librqbit engine ile public torrent iner, dosya çıkar, seed eder
- JSON-RPC köprü metodları PySpark yerine librqbit üzerinden aynı sözleşmeyi döner
- Python suite baseline değişmez

---

## İlerleme

- 2026-08-12 — Tanımlandı (Faz 10b blocker'ı aştı: aarch64 Linux sandbox)
- 2026-08-12 — Spike: manager-torrent crate derlendi (librqbit 8.1.1, aarch64); canlı
  Ubuntu torrent ~2.5 MiB/s indi (live_torrent.rs kanıtı)
- 2026-08-12 — Entegrasyon: `TorrentBackend` trait'i manager-bridge'e eklendi; mevcut
  `Bridge` (Python) bu trait üzerinden genelleşti; `AppState.bridge` → `Arc<dyn TorrentBackend>`.
  `LibrqbitEngine` (manager-torrent) Python `_BRIDGE_METHODS` sözleşmesini birebir taklit
  edip librqbit Session'ı lazy spawn ediyor. manager-bin `RGSX_TORRENT_ENGINE=librqbit`
  seçimini destekliyor (varsayılan Python bridge korunur). Test: 107/107 geçti
  (30 core + 5 bridge + 63 contract + 2 doctest + 7 manager-torrent).