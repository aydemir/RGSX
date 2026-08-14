# TASK-004 — Native katalog OTA bootstrap (boş kategori düzeltmesi)

- **id:** TASK-004
- **title:** Saf-Rust manager-bin kopyasında boş kategori — OTA veri çekme
- **status:** done
- **priority:** P0
- **created:** 2026-08-14
- **environment:** both
- **tags:** manager-http, manager-bin, catalog, native-catalog, ota

## Kaynak

- Kullanıcı bildirimi: "Retrobat kopyada canlıya alınınca webui kategoriler boş / sayfa boş"
- Teşhis: UI yükleniyor (statik dosyalar sorunsuz) ama `/api/platforms` boş dönüyor.

## Açıklama

### Problem
`RGSX rust.bat` saf-Rust `manager-bin`'i `RGSX_NATIVE_CATALOG=1` ile başlatır. Native
katalog (`manager-http/src/catalog.rs`), kategori (platform) listesini `RGSX_DATA_DIR`
(`saves/ports/rgsx`) içindeki `systems_list.json` + `games/<platform>.json` dosyalarından üretir.
`NativeCatalog::load_sources` (catalog.rs:248) bir platformı **yalnızca** karşılık gelen
`games/<ad>.json` dosyası varsa listeye alır.

Canlı/taze RetroBat kopyasında bu veri dosyaları henüz yoksa (Python manager hiç
çalıştırılmadıysa / OTA `games.zip` açılımı yapılmadıysa) `/api/platforms` `count:0` döner
→ WebUI'da kategori alanı tamamen boş görünür. `RGSX rust.bat` zaten "native catalog data
missing" uyarısı veriyordu ama yine de sunucuyu başlatıyordu; sonuç boş kategori.

Kök neden: Saf-Rust launcher "no Python required" dense de, native katalog verisi Python
tarafından (`rgsx_cli.ensure_data_present` → OTA `games.zip` indir + çıkar) üretiliyordu.
Yani Rust kopyası kendi verisini kendisi çekmiyordu.

### Çözüm
Python `ensure_data_present` mantığı birebir saf-Rust'a çevrildi:
`RGSX_NATIVE_CATALOG=1` modunda, başlangıçta `systems_list.json` + en az bir `games/*.json`
yoksa Rust, OTA `games.zip`'i indirip `RGSX_DATA_DIR`'e çıkarır. Böylece fresh kopya da
Python'a ihtiyaç duymadan kategorilerle açılır.

Zip URL çözümü (`get_sources_zip_url` eşleniği):
- `RGSX_SOURCES_MODE=custom` + `RGSX_SOURCES_ZIP_URL` (http/https) → o URL
- custom modunda URL boşsa `RGSX_DATA_DIR/games.zip` yerel dosyasına düş
- custom değilse `RGSX_SOURCES_ZIP_URL` veya varsayılan `https://retrogamesets.fr/softs/games.zip`

İndirme `reqwest` streaming, çıkarma `zip` crate ile (zip-slip korumalı `enclosed_name`).
Başarısızlıkta eski davranış korunur: native catalog yine boş kalır, sunucu yine de açılır
(sadece log/uyarı).

## Kapsam / Dosyalar

- **Yeni:** `manager-rs/manager-http/src/catalog_bootstrap.rs` — `ensure_catalog_ready()`
- **Değişen:** `manager-rs/manager-http/src/lib.rs` — `pub mod catalog_bootstrap;`
- **Değişen:** `manager-rs/manager-bin/src/main.rs` — native katalog dalında
  `ensure_catalog_ready().await` çağrısı (catalog kurulmadan önce)
- **Değişen:** `manager-rs/Cargo.toml` — `reqwest` features: `+ "stream"`
- **Değişen:** `manager-rs/manager-http/Cargo.toml` — `+ zip = { version = "2", features = ["deflate"] }`

## Doğrulama

- `cargo check -p manager-http` ✅ (yeni `zip` crate derlendi, koddam uyarı yok)
- `cargo check -p manager-bin` ✅
- Canlı senaryo: `systems_list.json` + `games/` olmayan bir `RGSX_DATA_DIR` ile
  `RGSX_NATIVE_CATALOG=1` başlat → başlangıçta OTA indirme/çıkarma logu, ardından
  `/api/platforms` dolu dönmeli.
- `catalog_present` kısa devre: veri zaten mevcutsa indirme atlanır (no-op).

## İlerleme

- 2026-08-14 — Kod yazıldı, `manager-http` + `manager-bin` `cargo check` ile doğrulandı.
