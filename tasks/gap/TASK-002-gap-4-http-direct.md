# TASK-002-gap-4 — HTTP-Doğrudan İndirme Alt Ağacı (Rust'ta eksik)

- **id:** TASK-002-gap-4
- **title:** HTTP-direct download path (vimm/archive.org/lolroms/1fichier, header variantları, browser-challenge, 429 backoff, Range resume, arşiv imza kontrolleri)
- **status:** todo
- **priority:** P0
- **created:** 2026-08-14
- **environment:** both
- **tags:** http, download, vimm, archive.org, lolroms, 1fichier
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `H0` .. `H12` (tüm HTTP-alt ağacı)
- `ports/RGSX/network/queue.py` `download_rom` HTTP dalı (satır 889–1527):
  - vimm.net: `_fetch_vimm_download_info`, `_get_vimm_file_size`, gerçek dosya adı çözümü
  - mevcut dosya kontrolü + boyut karşılaştırması (incomplete sil / already-present)
  - aynı taban farklı uzantı kontrolü
  - lolroms external tool (`_download_lolroms_with_external_tool`) + requests fallback
  - archive.org: cookie/metadata/alt-URL hazırlığı (`_try_archive_org_alternate_urls`)
  - HTTP retry döngüsü: header variantları, 401/403, **429 rate-limit backoff**,
    browser-challenge tespiti, timeout/connection retry
  - content-type HTML kontrolü (vimm), arşiv imza guards (`.7z/.zip/.rar`),
    kısmi arşiv kabul (`_should_accept_partial_archive`)
  - `_stream_response_to_path` (Range resume), `InsufficientDiskSpaceError`
- `ports/RGSX/network/one_fichier.py` — `download_from_1fichier` (ayrı modül, Rust'ta yok)

## Açıklama

Rust daemon'ı (`LibrqbitEngine` + `rust_daemon.download_torrent`) yalnızca **torrent**
(`source_url` magnet/`.torrent`) indirir. HTTP-doğrudan indirmeler (vimm.net, archive.org,
lolroms, 1fichier) Rust'ta **hiçbir karşılığa sahip değil**. Bu, tüm doğrudan-HTTP kaynak
indirmelerinin Python'da kalması gerektiği anlamına gelir; Rust refaktörü tamamlandığında
bu alt ağaç ya Rust'a taşınmalı ya da Python orkestratörü korunmalı. En büyük ve en nadir
dal içerikli düğüm kümesidir (browser-challenge, 429 backoff, archive alt-URL, imza guards).

## Kapsam / Dosyalar

- `manager-rs/manager-http/` veya yeni `manager-http-dl` crate — HTTP stream indirici
- `manager-rs/manager-torrent/src/lib.rs` — `download_torrent` sözleşmesine HTTP kaynak desteği
- `rust_daemon.py` — `_post_json("/api/download", {url})` zaten var; arşiv sonrası kontroller eklenmeli

## Doğrulama

- vimm.net: form/mediaId akışı + gerçek dosya adı çözümü Rust'ta çalışır.
- archive.org: cookie/metadata/alt-URL fallback zinciri parity.
- 429 rate-limit → Retry-After / exp backoff tekrar deneme.
- Browser-challenge tespitinde insan etkileşimi gerektiği doğru şekilde raise edilir.
- İndirilen `.7z/.zip/.rar` için HTML/challenge/imza guards + kısmi kabul kuralı birebir.
- Range resume: yarım dosya üzerinden devam (qBittorrent dışı HTTP için).
