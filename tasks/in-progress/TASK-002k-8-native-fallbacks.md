# TASK-002k-8 — Saf-Rust modda kalan placeholder artifact'ları native yap

- **id:** TASK-002k-8
- **title:** Pure-Rust modda (`catalog=None`) Python-artıfaktı dönen uçların native'i
- **status:** done
- **priority:** P2
- **created:** 2026-08-14
- **environment:** both
- **tags:** manager-http, native, system-info, browse, python-free
- **parent:** TASK-002k

## Arka plan
TASK-002k (Faz 10c/3) kapsamında manager-bin saf-Rust modda çalıştığında
(`RGSX_NATIVE_CATALOG=1`, Pythonsız) çoğu handler zaten Rust fallback'a düşüyor:
`scan` → `manager_scan`, `download`/`queue`/`progress`/`history` → Rust state,
`settings` → `manager_core::settings`, katalog → `NativeCatalog`. Geriye kalan
**görünür** Python-era placeholder'ları:

1. `GET /api/system_info` → `platforms_count: 0` (hardcoded). Saf-Rust modda yerel
   `systems_list.json` varsa gerçek platform sayısını vermeli.
2. `GET /api/browse-directories` (pathsiz) → `browse("")` boş döner; kök yerine
   `RGSX_DATA_DIR` (yoksa cwd) listelenmeli ki klasör seçimi çalışsın.

Not: `catalog` Python proxy'si hibrit mod için Tasarım gereği duruyor; saf-Rust
modda devreye girmez. Bu görev saf-Rust modu %100 Pythonsuz yapar.

## Adımlar
- **1.** `manager_core::settings::system_info()` → `count_native_platforms()` eklenir
  (`RGSX_DATA_DIR/systems_list.json` dizisi okunur, uzunluk `platforms_count`).
- **2.** `api.rs::browse_directories` → pathsiz çağrıda kök = `RGSX_DATA_DIR` (varsa
  ve dizinse) yoksa `.`.
- **3.** `cargo test -p manager-core` + `manager-http` build; contract 105 etkilenmez
  (Python modunda proxy yoluna düşer, fallback yolu sadece saf-Rust modda).

## Doğrulama
- `cargo test -p manager-core settings` 6/6 yeşil; `manager-bin` build yeşil.
- Canlı (`RGSX_NATIVE_CATALOG=1 RGSX_DATA_DIR=/tmp/rgsx_data`):
  - `/api/system_info` → `{"platforms_count":152,"roms_folder":"","system":"linux"}` (eskiden `platforms_count:0`/boş).
  - `/api/browse-directories` (pathsiz) → `current_path:/tmp/rgsx_data`, `games`+`images` alt dizinleri listelendi (eskiden boş).
- Contract 105 etkilenmez: Python hibrit modunda `system_info`/`browse` proxy yoluna düşer; değişiklikler yalnız saf-Rust fallback'inde.

## Sonuç & kapsam notu
Saf-Rust modda manager-bin zaten ~%100 Pythonsuzdu: `scan`→`manager_scan`,
`download`/`queue`/`progress`/`history`→Rust state, `settings`→`manager_core::settings`,
katalog→`NativeCatalog`. Geriye kalan görünür Python-era artifact'ları (system_info
count, browse kök) bu görevle native'lendi. `catalog` Python proxy'si yalnız hibrit
göç modunda devrede (Tasarım gereği); saf-Rust modda hiç Python çağrısı yok.
"manager-bin %100 Pythonsuz" hedefi native modda gerçekleşti. (Commit kullanıcı
isteğine bırakıldı.)
