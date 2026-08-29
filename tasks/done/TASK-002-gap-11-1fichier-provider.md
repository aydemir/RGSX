# TASK-002-gap-11 — 1fichier Provider Zinciri (Rust'ta eksik)

- **id:** TASK-002-gap-11
- **title:** 1fichier download provider zinciri (1F→AD→DL→RD→TB→FREE sıralı fallback, debrid unlock/poll, Range resume, 10x retry, provider_used history yazımı)
- **status:** done
- **updated:** 2026-08-28
- **priority:** P0
- **created:** 2026-08-14
- **environment:** both
- **tags:** 1fichier, alldebrid, debrid-link, realdebrid, torbox, download
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `OF0` .. `OF18` (tüm 1fichier subgraph)
- `ports/RGSX/network/one_fichier.py` — `download_from_1fichier` (451):
  - `OF0` API key'leri yükle/refresh (mtime aware): 1F/AD/DL/RD/TB
  - `OF1` duplicate URL → `url_done_events` bekle (≤1800s) → cache sonucu
  - `OF5→OF9` 1fichier direkt: `file/info.cgi` (filename+size) → `get_token.cgi` (final_url)
  - `OFA→OFT` fallback zinciri: AllDebrid `link/unlock` → Debrid-Link `downloader/add`
    → RealDebrid `unrestrict/link` → TorBox `webdl` (checkcached→createwebdownload→poll≤120s→requestdl)
  - `OFF` FREE mode: `download_1fichier_free_mode` (progress/wait callback, history yazımı)
  - `OFD2→OF15` boyut kontrolü + 10x retry döngüsü (3 header variantı, Range resume,
    AllDebrid 503 → `_refresh_alldebrid_final_url`)
  - `OF16→OF18` cancel/force_extract + `_finalize_download_result` bağlantısı
- `ports/RGSX/network/helpers.py` — `_is_ps3_redump_target`, `_postprocess_downloaded_file`
- `ports/RGSX/utils/api_keys.py` — `load_api_key_1fichier` (244), `load_api_key_alldebrid` (248), `load_api_key_debridlink` (252)

## Açıklama

1fichier indirmeleri `download_rom`'un HTTP dalından **tamamen ayrı** bir modülde (one_fichier.py)
çalışır ve Rust'ta **hiçbir karşılığı yok**. `gap-4` (HTTP-direct) vimm/archive.org/lolroms'u
kapsar; 1fichier ayrı bir indirme altyapısıdır:
  - 5 provider'a sıralı fallback (her birinin kendi hata kod haritası)
  - debrid URL'leri için HEAD size atlanması (geçici/tek kullanımlık URL)
  - FREE mode (API anahtarı yoksa) ayrı bir indirme akışı
  - `provider_used`/`provider_prefix` history alanları (UI'da "AD:" gibi gösterim)

Bu zincir Python kaldırıldığında davranışsal olarak kaybolur; Rust tarafına ya taşınmalı
ya da daemon/HTTP sözleşmesine eklenmelidir.

## Kapsam / Dosyalar

- `manager-rs/manager-http/` veya yeni crate — 1fichier/debrid provider istemcileri
- `rust_daemon.py` — `/api/download` sözleşmesine `provider` alanı / 1fichier route'u
- `tasks/gap/TASK-002-gap-4-http-direct.md` ile kapsam sınırı: gap-4 HTTP-direct genel,
  bu görev yalnızca 1fichier/debrid ailesi

## Doğrulama

- Provider fallback sırası 1F→AD→DL→RD→TB→FREE birebir korunur (hata kod haritaları dahil).
- `provider_used`/`provider_prefix` history'ye yazılır (UI parity).
- AllDebrid 503'te link yenileme (`_refresh_alldebrid_final_url`) çalışır.
- Range resume (`.part` + `os.replace`) + 10x retry/backoff parity.
- FREE mode: API anahtarı olmadan indirme + zip/rar/7z extract + progress callback'leri.
- Duplicate URL dedup (≤1800s bekleme + cache) korunur.

---

## İlerleme

- 2026-08-27 — Faz1: `manager-download/src/one_fichier.rs` (Provider enum 1F→FREE, ApiKeys env/file, DedupCache 1800s, history_provider_fields, 5 test) + `lib.rs` mod export, 34 lib + 14 http_integration yeşil
- 2026-08-28 — Faz2: `one_fichier.rs` OF5→OF9 1F direkt `file/info.cgi`→`get_token.cgi` (`onefichier_file_info`/`onefichier_get_token`/`onefichier_direct_url`, `OneFichierFileInfo`/`OneFichierDirectError`, `sanitize_filename`, 403/401/500 friendly msg, Resource not found, Bad token/Premium haritaları) + OFF FREE pure helpers (`extract_wait_seconds`, `extract_visible_text`, `normalize_1fichier_text`, `extract_free_block_reason`, `parse_free_form_data`, `extract_free_candidates`, html_unescape, upgrade advice), +11 test (wait ct_mul/min/sec, visible, normalize, block_reason×3, form, candidates, sanitize), Cargo `unicode-normalization`, 45 lib + 14 http_integration yeşil
- 2026-08-28 — Faz3: `one_fichier.rs` OFA→OFT debrid zinciri — `alldebrid_unlock`/`refresh_alldebrid_url` (GET link/unlock, success/data.link), `debridlink_add` (POST downloader/add, DEBRIDLINK_ERROR_MAP 16 kod), `realdebrid_unrestrict` (POST unrestrict/link, REALDEBRID_ERROR_MAP 10 kod + hoster_not_free→Premium), `torbox_webdl` (checkcached→createwebdownload→mylist poll ≤120s/3s→requestdl, TORBOX_ERROR_MAP 12 kod, DUPLICATE_ITEM→mylist hash fallback, md5_hex), `resolve_chain` (1F→AD→DL→RD→TB→FREE sıralı fallback), `DebridSuccess`/`DebridError`/`ChainOutcome`, Display for Provider, +5 pure test (debridlink/realdebrid/torbox map, md5, chain order), `md5` crate, 50 lib + 14 http_integration yeşil
- 2026-08-28 — Faz4: `one_fichier.rs` OFD2→OF18 final_url indirme motoru — `onefichier_header_variants` (3 variant browser→Accept:*/*→curl), `should_skip_head_for_provider` (AD/DL/RD atlama), `head_remote_size` (HEAD Content-Length, skip transient), `existing_file_status`/`find_same_stem_existing` (OF10 varlık+boyut+alternatif uzantı), `decide_force_extract` (is_zip_non_supported+auto || PS3 redump, extract::is_ps3_redump_target), `download_onefichier_final_url` (10x retry, variant rotasyon, Range resume via stream::resume_offset/.part, AD 503→refresh_alldebrid_url, disk::precheck_destination, stream::download_stream_async+finalize_part, cancel/progress), +5 test (header shape, skip_head, existing_status, same_stem, force_extract), 55 lib + 14 http_integration yeşil
- 2026-08-28 — Faz5: `one_fichier.rs` OFF FREE tam akış `free_mode_download` (GET→wait countdown+wait callback→f1 POST 3x extra-wait retry→extract_free_block_reason→extract_free_candidates→validate_free_candidate HEAD/GET→HEAD Content-Disposition filename+sanitize→stream .part+finalize, disk precheck, cancel/progress), `FreeModeError`, `extract_cd_filename`, `OneFichierOrchestrator::download` (resolve_chain→Debrid: existing check+download_onefichier_final_url / Free: free_mode_download, decide_force_extract→extract_archive, provider history), `manager-http/src/api.rs` `is_onefichier_url` + `download` 1fichier dispatch (iki nokta) + `native_onefichier_download` (kuyruk/semaphore/pause/network_down/retry envelope, provider history yazımı, finalize_download_in_state), `percent-encoding` decode, `should_force_extract` delegesi, 2 test (cd_filename, free_block), 57 lib + 14 http_integration + 28 http yeşil — **gap kapandı**