# TASK-002d — manager-windows: tray / autostart / firewall

- **id:** TASK-002d
- **title:** Windows entegrasyonu — tray ikonu, autostart registry, firewall
- **status:** todo
- **priority:** P2
- **created:** 2026-08-12
- **tags:** rust, windows-rs, tray, registry, firewall
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10: `windows-rs` (registry +
  firewall COM). Mevcut Python davranışı: `pystray` tray, `HKCU\...\Run\RGSXManager`
  autostart, firewall uygulama kuralı.

## Açıklama

Python manager'ın Windows-only parçalarını (`rgsx_manager.py`) Rust'ta ikame etmek;
Windows test imkânı mevcut olduğundan bu alt-görev aktif kapsamdadır.

## Kapsam / Dosyalar

- `manager-rs/manager-windows/src/lib.rs` — `cfg(windows)`:
  - **tray:** Windows tray ikonu + menu (pystray karşılığı; `tray-icon`/winapi)
  - **autostart:** `HKCU\...\Run\RGSXManager` registry yaz/sil (`windows-rs`)
  - **firewall:** uygulama kuralı yönetimi (COM `HNetCfg` / `netsh` karşılığı)
- `manager-rs/manager-bin/src/main.rs` — Windows'ta `cfg(windows)` tray/autostart/firewall bağlanır

## Doğrulama

- Windows'ta canlı: tray ikonu oluşur, menu tıklamaları çalışır
- Autostart: registry girişi yazılır/silinir (Kopya kurulum kullanılır, gerçek kurulum bozulmaz)
- Firewall: kural oluşturulur/kaldırılır (admin gerektiren parçalar netlenir)
- `cargo check --workspace` geçer; Python suite baseline değişmez

---

## İlerleme

- 2026-08-12 — Alt-görev tanımlandı (Windows ortamı kapsamında aktif)