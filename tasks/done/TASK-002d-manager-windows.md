# TASK-002d — manager-windows: tray / autostart / firewall

- **id:** TASK-002d
- **title:** Windows entegrasyonu — tray ikonu, autostart registry, firewall
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **environment:** windows
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
- 2026-08-12 — manager-windows: tray.rs (tray-icon + muda menu, ayrı thread + GetMessageW pump),
  autostart.rs (HKCU Run registry), firewall.rs (netsh advfirewall) eklendi; manager-bin main.rs'ye
  cfg(windows) setup + tray eylem döngüsü bağlandı.
- 2026-08-12 — **0xC0000139 fix:** `common-controls-v6` feature'ı muda'da boştur (manifest gömmez),
  yalnızca comctl32 v6-sadece `TaskDialogIndirect` import'unu açar → manifest'siz comctl32 v5.82
  yüklenince loader STATUS_ENTRYPOINT_NOT_FOUND verir. Feature kaldırıldı (muda MessageBoxW fallback).
  Canlı smoke: bridge + HTTP 5010 health + tray ikonu OK; autostart mevcut kaydı doğru tespit etti;
  firewall admin eksikliğini warn'a indirdi.
- 2026-08-12 — Tray OpenDownloads/OpenLogs: `get_app_paths` bridge method'u (qbittorrent_backend.py)
  üzerinden klasör yolları alınıp `explorer <path>` ile açılıyor. Rust workspace 96/96 test, Python
  baseline 9 pre-existing hata (Linux-only + pystray Windows ortamı) — regresyon yok.
- 2026-08-12 — **Tamamlandı.** Doğrulama: canlı smoke (tray + autostart + firewall + HTTP), cargo test
  --workspace, Python suite baseline.