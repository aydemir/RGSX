# TASK-002-gap-8 — Stray Torrent Temp-Root Temizliği (Rust'ta eksik)

- **id:** TASK-002-gap-8
- **title:** Stray .rgsx_torrent temp-root cleanup (eski platform path'lerinde orphan tarama)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-14
- **environment:** both
- **tags:** cleanup, torrent, temp
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümü: `Q9`
- `ports/RGSX/network/queue.py`:
  - `_find_stray_torrent_temp_roots(stable_key)` (134) — ROMS_FOLDER altındaki tüm platform
    klasörlerinde `.rgsx_torrent/<stable_key>` arar
  - `cleanup_torrent_temp` (161) ve `_cleanup_torrent_resume_artifacts` (195) bu taramayı kullanır

## Açıklama

Geçmişte platform klasör çözümü değiştiğinden, torrent temp kökleri eski bir platform
klasöründe "orphan" kalabiliyor. Python bunları `stable_key` (md5) ile tüm platform klasörlerinde
tarayarak temizler. Rust `LibrqbitEngine` sabit bir `output_folder` kullanır ve **orphan taraması
yapmaz**; eski konumlardaki `.rgsx_torrent` klasörleri birikir.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — cleanup'ta stray tarama (`_find_stray_torrent_temp_roots` karşılığı)

## Doğrulama

- Farklı bir platform path'inde kalan `.rgsx_torrent/<key>` klasörü Rust cleanup ile silinir.
- Mevcut (aktif) temp köküne yanlışlıkla dokunulmaz.

---

## Parite Denetimi 2026-08-15 — Ek Maddeler

### Madde A: Disk yazma izolasyonu + kaynak koruma eksik (⚠️ KISMİ — BELİRSİZ)

- Python: `qbittorrent_backend.py:1556` savepath `.rgsx_torrent/<key>` (temp izolasyonu);
  `:1675-1681` kaynağı koruyup yalnız seed için `os.link` (orijinal silinmez).
- Rust: `manager-torrent/src/lib.rs:73` `output_folder`; `:321-368` en büyük dosyayı seçip
  `link_or_copy` — **temp_dir izolasyonu ve seed için kaynak koruma YOK**.
- BELİRSİZ: izolasyon stratejisi (ayrı temp root mu, mevcut `output_folder` mı?) ve kaynak koruma
  yaklaşımı kullanıcı onayı gerektirir. `TASK-002-gap-16` ile `lib.rs` paylaşımı var.
