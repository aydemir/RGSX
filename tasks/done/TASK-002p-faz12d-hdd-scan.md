# TASK-002p — Faz 12d: HDD scan native port (Python→Rust)

> Bağımlı: ROADMAP_FAZ12_RUST_WEBUI_TVUI.md (onaylandı). Python `update_gamelist.py` /
> `update_gamelist_windows.py` + `utils/history_matches.py` + `ROMS_FOLDER` tarama
> mantığı native Rust'a taşındı (yeni `manager-scan` crate).

## Uygulama (`manager-rs/manager-scan`)
- `scan`: `walkdir` ile `ROMS_FOLDER` tarama → platform klasörlerine göre ROM dosyaları
  (uzantı filtresi), boyut toplamı. ROM içermeyen klasörler elenir.
- `disk`: `sysinfo` ile disk kullanımı (total/used/free).
- `gamelist`: `quick-xml` ile `gamelist.xml` oku/yaz. Linux (yalnız RGSX entry) ve
  Windows (mevcut entry'leri koruyup merge) varyantları; `RGSX_ENTRY_LINUX`/`WINDOWS`.
- `history`: `history_matches.py` portu — `local_path`/`moved_paths` varlık kontrolü.
- `manager-http`: `/api/scan` endpoint'i (`RGSX_ROMS_FOLDER` env) → JSON + SSE `scan` olayı.

## Doğrulama
- `cargo test -p manager-scan` → 8 test yeşil (scan grup, gamelist linux/windows merge,
  history match, disk). `cargo test -p manager-http` → 102 contract yeşil.
- Runtime: `/api/scan` → platforms gruplu + disk kullanımı (persistent fixture ile).

## Bilinen sapmalar
- gamelist.xml çıktısı compact (Python minidom tab-indent üretir; sözleşme yok).
- `scan` ROM uzantı listesi örnek; Python `config`'a göre genişletilebilir.
