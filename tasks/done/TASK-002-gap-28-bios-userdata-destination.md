# TASK-002-gap-28 — BIOS kategori indirme/çıkarma hedefi parity (USERDATA_FOLDER yönlendirmesi)

- **id:** TASK-002-gap-28
- **title:** BIOS kategorisi (ör. "- BIOS by TMCTV -") indirme + çıkarma hedefinin USERDATA_FOLDER'a yönlendirilmesi — Rust/Python parity
- **status:** done
- **priority:** P2
- **created:** 2026-08-18
- **environment:** both (Linux/Batocera + Windows)
- **tags:** download, extract, bios, parity, paths
- **parent:** TASK-002

## Karar (2026-08-18)

Kullanıcı sordu: "BIOS by TMCTV kategorisindeki zipler nereye açılıyor, Rust/Python kontrolü?".
Kod incelemesiyle tespit edildi:

- **Python** BIOS ziplerini `config.USERDATA_FOLDER`'a yönlendirir (roms alt klasörü DEĞİL).
  - `network/queue.py:770-771`: `if platform_folder=="bios" or platform=="BIOS" or platform=="- BIOS by TMCTV -": dest_dir = config.USERDATA_FOLDER`
  - `network/queue.py:1609`: `_postprocess_downloaded_file(dest_path, dest_dir, ...)` → içerik `USERDATA_FOLDER`'a açılır.
  - `config.py`: `USERDATA_FOLDER = dirname(dirname(dirname(APP_FOLDER)))` (3 seviye yukarı).
    - **Linux/Batocera** (traditional): `APP_FOLDER = /userdata/roms/ports/RGSX` → `USERDATA_FOLDER = /userdata`.
    - **Windows** (traditional): aynı formül; `APP_FOLDER = <install_root>/RGSX` → 3 seviye yukarı (kurulum konumundan türeyen, sabit literal yok).
    - **Docker**: `USERDATA_FOLDER = DATA_FOLDER = _docker_data_dir`.
- **Rust** bu yönlendirmeyi **yapmıyor**. İndirme+çıkarma her zaman
  `<RGSX_ROMS_FOLDER>/<platform_folder>/<name>` (ve çıkarma `<RGSX_ROMS_FOLDER>/<platform_folder>/`)
  altına düşüyor (`api.rs:530-535`, `platform_folder_for` api.rs:1946, `dest_dir = dest_path.parent()`
  `manager-torrent/src/lib.rs:235`). BIOS-LIKE algılama var (`extract.rs:56` `"- BIOS by TMCTV -"`
  kümesinde) ve `should_force_extract` ile açma ZORLANIYOR, ama **hedef klasör yanlış** (roms altı).

## Python Referans Davranışı

- `ports/RGSX/network/queue.py:770-771` — BIOS platformları için `dest_dir = config.USERDATA_FOLDER`.
- `ports/RGSX/network/queue.py:1575-1582` — `bios_like = {"BIOS","- BIOS by TMCTV -","- BIOS"}`;
  auto_extract açıkken BIOS için force_extract=true.
- `ports/RGSX/network/queue.py:1609` — `_postprocess_downloaded_file(dest_path, dest_dir, ...)`.
- `ports/RGSX/config.py` — `USERDATA_FOLDER = dirname(dirname(dirname(APP_FOLDER)))`.

## Rust Mevcut Durum (❌ parity açığı)

- `manager-http/src/api.rs:530-535` + `platform_folder_for` (api.rs:1946): BIOS için özel dal YOK →
  hedef `<RGSX_ROMS_FOLDER>/<platform_folder>/`.
- `manager-torrent/src/lib.rs:235`: `dest_dir = dest_path.parent()` → roms alt klasörü.
- `manager-core/src/extract.rs:56` `BIOS_LIKE` listesinde `"- BIOS by TMCTV -"` var (açma zorlanır)
  ama hedef yönlendirmesi yok.
- Rust'ta `USERDATA_FOLDER` kavramı veya Windows/Linux'a özel BIOS hedefi YOK.

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-http/src/api.rs` `download` handler — BIOS platformları için `dest_dir`'i
  USERDATA_FOLDER eşdeğerine yönlendir (Python `queue.py:770` parity).
- Yeni resolver: `userdata_folder() -> Option<PathBuf>` — env `RGSX_USERDATA_FOLDER` > türetme
  (`RGSX_DATA_DIR`'dan 3 seviye yukarı, veya `RGSX_ROMS_FOLDER`'dan 1 seviye yukarı → `/userdata`).
- Windows (`manager-windows` / NSIS) için eşdeğer USERDATA yolu tanımlanmalı (3-seviye-yukarı formülü
  Windows kurulum köküne uygulanır).

## Bağımlılık

- `TASK-002-gap-8` (stray-temp reaper) ve `TASK-002-gap-12` (FIFO) ile ilgisiz; bağımsız.
- `manager-core/src/settings.rs` `roms_folder` ile etkileşim (effective_roms_folder).

## Doğrulama

- BIOS zip'i Linux'ta `/userdata` (veya RGSX_USERDATA_FOLDER) altına, Windows'ta Windows eşdeğerine iner/açılır.
- Parity: Python `queue.py:770` dalı ile birebir (BIOS → USERDATA_FOLDER, roms altı DEĞİL).
- Contract/unit test: `platform == "- BIOS by TMCTV -"` → dest_dir USERDATA eşdeğeri.

## 2026-08-18 UYGULANDI

`manager-http/src/api.rs`:
- `userdata_folder()` resolver'ı eklendi: `RGSX_USERDATA_FOLDER` env > `RGSX_DATA_DIR` 3 seviye
  yukarı > `RGSX_ROMS_FOLDER` 1 seviye yukarı. Hiçbiri yoksa `None` (redirect atlanır).
- `redirect_bios_dest()` eklendi: `is_bios_platform(platform_folder_for(p), p)` true ve
  `userdata_folder()` Some ise dest_path = `USERDATA/<sanitized_name>`; değilse roms altı kalır.
- İki indirme yoluna uygulandı (torrent/bridge yolu ~528 ve native HTTP yolu ~1684); `download_batch`
  zaten `download`'a delegasyon yaptığından otomatik kapsanır.
- Çıkarma hedefi zaten `dest_path.parent()` (`manager-torrent/src/lib.rs:235`) olduğundan, dest_path
  USERDATA'ya kaydırılınca BIOS zip'inin **içeriği de otomatik USERDATA'ya açılır** (Python parity).

Testler: `api::tests::gap28_bios_redirects_dest_to_userdata` + `gap28_non_bios_keeps_roms_dest`
geçti; tam manager-http suite (contract 113 + lib) yeşil. Windows eşdeğeri `RGSX_USERDATA_FOLDER`
env'iyle NSIS kurulumunda ayarlanır (kod tarafı platform-bağımsız).
