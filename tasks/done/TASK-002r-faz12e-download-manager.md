# TASK-002r — Faz 12e: download manager (native DDL resolver + doğrudan HTTP indirme)

> Bağımlı: ROADMAP_FAZ12_RUST_WEBUI_TVUI.md. Python `one_fichier.py` / `utils/torrent.py`
> DDL/debrid çözüm mantığı native Rust'a taşındı (`manager-download` crate + `manager-http`
> native DDL dalı). Torrent indirme zaten librqbit üzerinden native (Faz 10c).

## Uygulama
- `manager-rs/manager-download`: `Resolver` trait + `DirectResolver` (torrent/DDL sınıflandırma),
  `OneFichierResolver`/`RealDebridResolver` (`RGSX_1FICHIER_KEY`/`RGSX_REALDEBRID_KEY` ile;
  anahtar yoksa `NotConfigured`, varsa `NotImplemented` — ağ gerektirir), `DownloadManager`
  zinciri (debrid→Direct). 3 birim test yeşil.
- `manager-http/src/api.rs`: `RGSX_NATIVE_DOWNLOAD=1` ile `/api/download` DDL dalı →
  `DownloadManager::resolve` → `DirectHttp` ise reqwest ile indirir, `downloaded`/history/
  progress + SSE ile sonuçlanır (Python proxy'si devre dışı). Torrent dalı librqbit'te kalır.

## Doğrulama
- `cargo test -p manager-download` → 3/3 yeşil. `cargo test -p manager-http` → 102 contract + 5
  birim yeşil (flag kapalıyken davranış değişmez).
- Runtime: yerel HTTP sunucu + `RGSX_NATIVE_DOWNLOAD=1` → `/api/download` `Download_OK`,
  `progress:100` (dosya bridge downloads klasörüne yazıldı). Python'a bağımlılık kalmadı.

## Ertelenen (ROADMAP "atlanan" listesinden)
- Debrid gerçek link çözme (1Fichier/RealDebrid ağ çağrısı) — `Resolver` arayüzü hazır,
  `NotImplemented` dalı doldurulacak.
- UPnP/IGD, i18n (fluent), OTA update, settings şeması, controls mapper, gamepad input —
  ayrı alt görevler; `display/*` retire sonrası kapsam.
