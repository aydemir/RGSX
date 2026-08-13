# TASK-002o — Faz 12c: Catalog native port (Python→Rust)

> Bağımlı: ROADMAP_FAZ12_RUST_WEBUI_TVUI.md (onaylandı). `CatalogSource` trait
> (`manager-http/src/catalog.rs`) zaten var; `PythonCatalog` proxy'si onu dolduruyor.
> Bu görev `NativeCatalog` ekler: aynı local dosyaları (systems_list.json, games/*.json,
> languages/, images/) okuyup birebir aynı JSON şeklini üretir → Python'sız catalog.

## Sözleşme (codegraph ile Python'dan çıkarıldı)
- `/api/platforms` → `{success,count,platforms:[{platform_name,folder,platform_image,...,games_count}]}`
- `/api/search?q=` → `{success,search_term,results:{platforms:[...],games:[{game_name,platform,url,size,downloaded}]}}`
- `/api/games/:platform` → `{success,platform,count,games:[{name,url,size,downloaded}]}`
- `/api/translations` → `{success,language,translations:{...}}`
- `/api/image/:platform` → box-art ham bayt (+ content-type)

## Uygulama
- `NativeCatalog` (`catalog.rs`): `load_sources` (systems_list.json normalize + runtime
  game-file filtresi), `load_games` (list veya {games:[...]}, dict/list/tuple), translations
  (languages/<lang>.json), image (images/<platform>.{png,jpg,webp}).
- Komut POST'ları (`/api/download`,`/api/queue`,...) native değildir → `NativeCatalog`
  içindeki opsiyonel `PythonCatalog` fallback'e proxy edilir (downloads Faz 12e'ye kaldı).
- `RGSX_NATIVE_CATALOG=1` → main.rs `NativeCatalog` kurar (yollar `RGSX_DATA_DIR` altından).

## Bilinen sapmalar (kabul edildi)
- Torrent kaynak genişletmesi (`_expand_torrent_source`) native'de yok → torrent tabanlı
  oyunların `url`'i None kalır (indirme Faz 12e'de librqbit ile zaten native).
- `downloaded` alanı native'de false (history taraması Faz 12d'ye kaldı).
- `config.filter_platforms_selection` native'de yok (varsayılan tümü görünür).

## Doğrulama
- `catalog.rs` içi birim testleri: fixture sources+games+languages+image ile şekil assert.
- `cargo test -p manager-http` → contract yeşil kalır (PythonCatalog default korunur).
- Runtime: `RGSX_NATIVE_CATALOG=1 RGSX_DATA_DIR=...` → `/api/platforms` birebir.
