# TASK-002-gap-6 — Arşiv Auto-Extract / Post-Process (Rust'ta eksik)

- **id:** TASK-002-gap-6
- **title:** Archive auto-extract / post-processing (BIOS, PS3 redump force extract)
- **status:** todo
- **priority:** P2
- **created:** 2026-08-14
- **environment:** both
- **tags:** extract, postprocess, bios, ps3
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `H12`, `H12e`
- `ports/RGSX/network/queue.py` `download_rom` (satır 1570–1615):
  - `get_auto_extract()` ayarı
  - `is_zip_non_supported` VEYA BIOS platform VEYA PS3 redump (`_is_ps3_redump_target`) → `force_extract`
  - `force_extract` → status "Extracting" → `_postprocess_downloaded_file(dest_path, dest_dir, url, game_name, is_ps3_target)`

## Açıklama

Bazı platformlarda (BIOS, PS3 redump, `is_zip_non_supported`) indirilen arşiv otomatik
açılmalıdır. Bu, indirme sonrası zorunlu bir post-process adımıdır. Rust
`LibrqbitEngine.download_torrent_source` indirip `link_or_copy` yapar, **extract yapmaz**.
HTTP path'inde de (`H12`) aynı mantık Python'da kalır.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — indirme sonrası extract kancaları
- `manager-rs/manager-core/src/` — BIOS/PS3 redump kuralı (mevcut `helpers._is_ps3_redump_target` karşılığı)

## Doğrulama

- BIOS/PS3 redump torrent: indirme sonrası otomatik extract gerçekleşir, history "Extracting"→tamam.
- `is_zip_non_supported` ayarı parity.
- Extract hatası → FAILED_PERMANENT (state.rs transition zaten var).
