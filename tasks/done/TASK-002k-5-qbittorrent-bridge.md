# TASK-002k-5 — Faz 10c/3/5: qBittorrent bridge handler'ları Rust'e

- **id:** TASK-002k-5
- **title:** `change_password`/`qb_start`/`qb_password_status` → `TorrentBackend` trait metotları
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, qbittorrent, bridge
- **parent:** TASK-002k

## Açıklama

`TorrentBackend` trait'inde zaten `get_password_status`/`change_webui_password`/`ensure_running`
var (manager-bridge/lib.rs:310-322). Rust `api.rs` handler'ları (`change_password`, `qb_start`,
`qb_password_status`) bu metotları bridge'e bağlar; Python yolu (`RGSX_TORRENT_ENGINE=python`)
için `Bridge::call` ile qbittorrent_backend.py'ye proxy eder. WebUI URL (`get_webui_url`) de
bağlanır. Bu, Faz 10b'de bilinçli korunan qBittorrent WebUI/şifre/migration yolunu Rust yüzeyine
taşır.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` (3 handler)
- `manager-rs/manager-bridge/src/lib.rs` (trait metotları yeterli; proxy doğrula)
- `manager-rs/manager-http/tests/contract.rs`

## Doğrulama

- `cargo test -p manager-http` + `tests/test_qbittorrent_backend.py`/`test_password_migration.py` yeşil.
- `RGSX_TORRENT_ENGINE=python` ile qBittorrent WebUI şifre akışı bozulmaz.

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
