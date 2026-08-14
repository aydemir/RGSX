# TASK-002-gap-9 — Restart Sonrası Yarıda Kalan İndirmeyi Sürdürme (Rust'ta eksik)

- **id:** TASK-002-gap-9
- **title:** Interrupted-download resume after restart (Rust librqbit session ephemral)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** resume, restart, torrent
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümü: `M0`
- `ports/RGSX/rgsx_manager.py` `_resume_interrupted_downloads` (743):
  - başlangıçta history(Téléchargement/Downloading/Paused) → queue'ya "Queued" geri ekler
  - yorum: torrentler qBittorrent partial'ı korur → kaldığı yerden devam; HTTP baştan
- `ports/RGSX/qbittorrent_backend.py`: `download_torrent_via_qbittorrent` (qBittorrent partial resume)

## Açıklama

Uygulama yeniden başladığında Python, yarım kalan indirmeleri queue'ya geri koyar. qBittorrent
path'inde torrent partial verisi korunur ve kaldığı yerden devam eder. Rust `LibrqbitEngine`
session'ı **ephemral** (in-process, persistans yok): manager yeniden başlayınca librqbit session
kaybolur, torrent partial verisi ve resume mümkün olmaz → indirme baştan başlar veya kaybolur.
`manager-torrent` `Session::new(output_folder)` ile kurulur ama state复活 yok.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — librqbit session persistans + resume
- `manager-rs/manager-bin/src/` — restart sonrası aktif indirmeleri yeniden devretme

## Doğrulama

- Manager restart → Rust daemon aktif torrentleri korur/RESUME eder (qBittorrent parity).
- Partial `.rqbitpart` verisi restart sonrası kaybolmaz.
- `_resume_interrupted_downloads` mantığı Rust daemon'a taşınır veya Python orkestratör korunur.
