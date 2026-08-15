# TASK-002-gap-26 — manager-bin path-resolution (exe'den türetme + env override)

- **id:** TASK-002-gap-26
- **title:** manager-bin path-resolution (current_exe türetme + env override fallback)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** manager-bin, paths, launcher, refactor
- **parent:** TASK-002

## Karar (2026-08-15) — onaylanan plan

- `manager-bin`, tüm path env'lerini (`RGSX_WEBUI_DIR`, `RGSX_DATA_DIR`, `RGSX_LANGUAGES_FOLDER`,
  `RGSX_DOWNLOADS_FOLDER`, `RGSX_LOGS_FOLDER`, `RGSX_ROOT`, `RGSX_MANAGER_SCRIPT`) kendi
  `current_exe()` konumundan türetir. Desen: **env varsa öncelik ver, yoksa exe'den türet, panic ETME.**
- Launcher (`RGSX rust.bat`) bu path env'lerini set etmeyi bırakır; yalnız runtime flag'ları
  (`RGSX_TVUI`, `RGSX_DISPLAY`, `RGSX_WINDOWED`, `--windowed`/`--no-tvui`) + exe yolu + kendi log'ları kalır.
- `RGSX_ROOT` Rust'ta OKUNMUYOR → launcher'dan düşürülür.
- Türetilen değerler `std::env::set_var` ile geri yazılır (downstream `env::var` okuyan
  `catalog.rs:189`, `catalog_bootstrap.rs:23`, `api.rs:323`, `settings.rs:29/278`,
  `catalog.rs:194` bunları böyle alır) → bu crates'ler DEĞİŞMEZ.

## ZORUNLU SIRA (çağrı sırası)

`resolve_paths()` **`main()`'in EN BAŞINDA, tokio runtime / herhangi bir thread spawn EDİLMENDEN
ÖNCE** çağrılmalı. Gerekçe: `std::env::set_var` thread-safe DEĞİL (Rust 1.80+ `unsafe`).

**Doğrulama (kod üzerinden):** `main` = `#[tokio::main] async fn main()` (`manager-bin/src/main.rs:187-188`).
Tüm env okumaları gövdenin senkron prolog'unda, ilk `tokio::spawn`'tan (`main.rs:296`, tray task,
`run_with_tray`@278 içinde) ve `axum::serve().await` (`main.rs:280/281/340`) ÖNCE:
- `RGSX_MANAGER_SCRIPT` `main.rs:28` (→`resolve_script`@203)
- `RGSX_TORRENT_ENGINE` `main.rs:50`, `RGSX_DOWNLOADS_FOLDER` `:69`, `RGSX_LOGS_FOLDER` `:71` (→`resolve_engine`@204)
- `RGSX_NO_AUTOSTART` `main.rs:105` (→`setup_windows`@275)
- `RGSX_RUST_WEBUI` `:194`, `RGSX_MANAGER_BIN_PORT` `:196`
- `RGSX_WEBUI_DIR` `:210`
- `RGSX_NATIVE_CATALOG` `:224`, `RGSX_PYTHON_MANAGER_URL` `:232`
- `RGSX_NATIVE_INPUT` `:252`, `RGSX_TVUI` `:260`

Bugün concurrency YOK (hepsi tek async task içinde) ama yine de runtime öncesi olmalı.

**Uygulama şekli (zorunlu):**
`#[tokio::main] async fn main()` YERİNE senkron `fn main()`:
```rust
fn main() {
    manager_bin::resolve_paths(); // unsafe { set_var } — TEK thread, runtime ÖNCESİ
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async { /* mevcut main gövdesi */ });
}
```
`set_var` çağrıları `unsafe { std::env::set_var(k, v) }` ile sarılır; yorumda "single-threaded,
runtime öncesi" garantisi belgelenir. Sadece override YOKSA yazılır (`if std::env::var_os(k).is_none()`).

## KAPSAM DIŞI (çift-"roms" off-by-one)

`.bat`'taki `ROOT_DIR = SCRIPT_DIR\..\..` + tekrar `\roms\ports\RGSX` hesabının olası off-by-one'ı
(**çift `roms`**) BU TASK'IN KAPSAMINDA DEĞİL. Ayrı hızlı doğrulama gerekir: gerçek RetroBat
kurulumunda `.bat` çalıştırılıp `Rust_RGSX_log.txt`'e yazılan `ROOT_DIR`/`RGSX_DATA_DIR` değerlerinin
gerçek path'e işaret edip etmediği kontrol edilmeli. **Bu refactor'a KARIŞTIRILMAYACAK.**
➡️ Eğer bug doğrulanırsa KOŞULLU olarak **ayrı TASK `gap-28` (bug-fix)** açılacak. Bu ortamda
gerçek RetroBat kurulumu çalıştırılamadığından `gap-28` ŞİMDİLİK AÇILMADI; yalnızca burada not edildi.

## Referans (mevcut davranış / RetroBat ağacı)

- `windows/RGSX rust.bat:141-152,203-214` — launcher'ın set ettiği path env'leri (kaldırılacak).
- `windows/RGSX rust.bat:89` — `ROOT_DIR = SCRIPT_DIR\..\..` (kapsam dışı, bkz. KAPSAM DIŞI).

## Rust Mevcut Durum (dosya:satır)

- `manager-bin/src/main.rs:27-40` `resolve_script()` — CWD'ye göreli `../ports/RGSX/...` fallback
  (exe'ye değil; kırılgan). `RGSX_MANAGER_SCRIPT` override.
- `manager-bin/src/main.rs:210-219` `static_root` — `RGSX_WEBUI_DIR` override + script-parent/`static`.
- `manager-bin/src/main.rs:69-72` downloads/logs — yalnız librqbit dalında, `temp_dir()` default.
- `manager-http/src/catalog.rs:189` `RGSX_DATA_DIR` (default `"."`), `:194` `RGSX_LANGUAGES_FOLDER`.
- `manager-http/src/catalog_bootstrap.rs:23` `RGSX_DATA_DIR`.
- `manager-http/src/api.rs:323` `RGSX_DATA_DIR`, `:415` `RGSX_NATIVE_DOWNLOAD`.
- `manager-core/src/settings.rs:29,278` `RGSX_DATA_DIR` (default `"."`).

## Kapsam / Dosyalar (değişecek)

- `manager-bin/src/paths.rs` (YENİ) — `struct RgsxPaths`; `fn resolve_paths()`; anchor tabanlı
  `root` bulucu (exe dizininden yukarı, `roms/ports/RGSX` imzası; yoksa 3×`.parent()` fallback);
  her alan için env-override öncelikli türetme; `set_var` yardımcısı (`apply`).
  **Anchor tespiti başarısız olup 3×`.parent()` fallback'e düşülürse `tracing::warn!` ile net mesaj
  basılır:** `path anchor (roms/ports/RGSX) bulunamadı, fallback (.parent×3) kullanılıyor: {path}`.
  Gerekçe: gap-27 ile `RGSX_NATIVE_CATALOG`/`RGSX_NATIVE_DOWNLOAD` default'ları `true` olduğundan,
  yanlış path artık sessizce Python proxy'ye düşmez; log'suz hata boş katalog/eksik dosya olarak
  geç ve kafa karıştırıcı şekilde ortaya çıkar — fallback anında basılan warn, kök sebebi görünür kılar.
- `manager-bin/src/main.rs` — senkron `fn main()` + manuel runtime (ZORUNLU SIRA); `resolve_paths()`
  ilk satır; `resolve_script()` `rgsx_dir`'den türetir (sibling `qbittorrent_backend.py`);
  `static_root` env-override + türetme.
- `windows/RGSX rust.bat` — `141-152,203-214,216-217` (path/data/catalog/torrent/autostart set +
  mkdir) satırları düşürülür; `RGSX_DISPLAY`/`RGSX_TVUI`/exe-yolu/log korunur (bkz. gap-27).
- `manager-http/*`, `manager-core/*` — DEĞİŞMEZ (env ile beslenir).

## Bağımlılık

- `TASK-002-gap-27` (flag defaults) ile birlikte uygulanmalı (aynı `main` prolog'u).
- KAPSAM DIŞI `gap-28` (çift-roms) — bu TASK'a bağımlı DEĞİL, ayrı yürür.

## Doğrulama

- Launcher'sız `manager-bin.exe` (sadece exe, env'siz) doğru `webui/saves/ports/rgsx` yollarını
  türetir; `GET /api/health` + webui yüklenir.
- `RGSX_WEBUI_DIR` vb. manuel set edilince override onurlandırılır (geriye dönük uyum).
- `cargo build` + `RUST_LOG=info` ile `resolve_paths` logları türetilen yolları doğru basar.
- Anchor tespiti zorlanmış senaryoda (yanlış exe konumu / taşınmış kurulum) `tracing::warn!` ile
  `path anchor (roms/ports/RGSX) bulunamadı, fallback (.parent×3) kullanılıyor: {path}` log'u basılır.
