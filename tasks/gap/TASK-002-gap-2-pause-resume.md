# TASK-002-gap-2 — Pause/Resume Orkestrasyonu (Rust'ta eksik)

- **id:** TASK-002-gap-2
- **title:** Pause/Resume orchestration (toggle, pause_all, resume_all, pause_ev → backend)
- **status:** todo
- **priority:** P0
- **created:** 2026-08-14
- **environment:** both
- **tags:** pause, resume, systray, download
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `P0`, `P1`, `P2`, `P7`
- `ports/RGSX/network/queue.py`:
  - `toggle_pause_download` (satır 288) — event yoksa oluşturur, set/clear döndürür
  - `is_download_paused` (306), `pause_all_downloads` (333), `resume_all_downloads` (361),
    `is_any_download_paused` (387)
  - `pause_events` dict'i `qbittorrent_backend.download_torrent_via_qbittorrent`'e `pause_ev`
    olarak geçer; döngü içinde `pause_ev.is_set()` → qBittorrent API pause/resume (`P7`)
- Kullanıcı talebi (hafıza): systray "indirme duraklatma" özelliği bu düğüme bağlı

## Açıklama

Pause/resume şu an tamamen Python `pause_events` threading.Event sözlüğü üzerinden yürür.
qBittorrent path'inde `pause_ev` gerçek qBittorrent API pause/resume'e bağlanır (sadece
polling durmaz, torrent askıya alınır). Rust `LibrqbitEngine.download_torrent_source`
(`manager-torrent/src/lib.rs`) **pause/resume'i hiç desteklemiyor** — `wait_until_completed()`
ile bloklar, iptal/duraklatma sinyali yok. Systray duraklatma özelliği Rust torrent path'inde
çalışmaz.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — pause/resume kancaları (librqbit `ManagedTorrent` pause API)
- `manager-rs/manager-bin/src/` — `/api/pause`, `/api/resume` uçları + `pause_all`/`resume_all` toplu kontrol
- `rust_daemon.py` — `download_torrent` içinde `pause_ev` benzeri sinyalin Rust'a iletilmesi

## Doğrulama

- `toggle_pause_download` davranışı (event oluşturma + set/clear dönüşü) Rust daemon'da yeniden üretilir.
- `pause_all_downloads`/`resume_all_downloads`: aktif torrentler + history durumu senkron güncellenir.
- qBittorrent fallback path'inde mevcut `P7` davranışı korunur (regression yok).
- Systray "duraklat" tıklanınca Rust torrenti gerçekten askıya alır (test: speed → 0).
