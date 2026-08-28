# TASK-002-gap-11 — 1fichier Provider Zinciri (Rust'ta eksik)

- **id:** TASK-002-gap-11
- **title:** 1fichier download provider zinciri (1F→AD→DL→RD→TB→FREE sıralı fallback, debrid unlock/poll, Range resume, 10x retry, provider_used history yazımı)
- **status:** in-progress
- **updated:** 2026-08-27
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