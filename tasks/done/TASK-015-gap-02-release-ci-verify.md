# TASK-015-gap-02 — Release CI gerçek koşu doğrulaması (native Rust pipeline)

- **id:** TASK-015-gap-02
- **title:** `.github/workflows/release.yml` Rust pipeline'ının ilk gerçek tag koşusuyla doğrulanması
- **status:** done
- **priority:** P1
- **created:** 2026-08-26
- **environment:** both
- **tags:** release-ci, github-actions, sdl2, packaging

## Kaynak

- TASK-012-gap-02 (`tasks/done/TASK-012-gap-02-python-cleanup.md`) — release.yml Python
  paketlemesinden Rust binary + WebUI paketlemesine çevrildi; ancak yalnız statik olarak
  yazıldı, GitHub Actions üzerinde hiç koşulmadı.

## Açıklama

Yeni pipeline üç job kullanır: `build_webui_linux` (npm build + Linux x86_64
manager-bin), `build_windows` (MSVC manager-bin.exe), `create_release` (artifact'leri
birleştirip 3 zip + release notes + GitHub Release + Discord webhook). Şu varsayımlar
**tahminîdir ve ilk koşuda kırılabilir**:

1. **SDL2 bundled apt listesi** (ubuntu-22.04): `build-essential cmake ninja-build
   pkg-config libx11-dev libxext-dev libxrandr-dev libxcursor-dev libxinerama-dev
   libxi-dev libgl1-mesa-dev`. `sdl2-sys` bundled CMake derlemesinin istediği ek
   paket varsa workflow'a eklenmelidir.
2. **windows-latest'te cmake/MSVC ön kurulu** varsayımı (GH runner imajında mevcut).
3. **glibc uyumu:** Linux binary ubuntu-22.04'te derlenir (eski glibc); Batocera/Knulli
   cihazda `ldd manager-bin` ile doğrulanmalı.
4. **Zip izinleri:** `RGSX.sh` + `manager-bin` executable bit'i `zip` CLI ile korunur;
   Windows'ta açılıp Batocera'ya kopyalanan pakette `chmod +x` gerekebilir (release
   notes'a not düşülmesi gerekebilir).
5. Artifact adları (`rgsx-linux`, `rgsx-webui`, `rgsx-windows`) ve `download-artifact@v4`
   dizin yapısı (`artifacts/rgsx-linux/RGSX/...`) birleşik.

## Kapsam / Dosyalar

- `.github/workflows/release.yml` (düzeltmeler burada)
- Gerekirse release notes metni (kurulum talimatı güncellemeleri)

## Doğrulama

- Test tag'i push et (örn. `v9.9.9-test`, sonrasını sil): tüm 3 job yeşil.
- Üç zip indirilir; içerik listesi doğrulanır:
  - `RGSX_update_latest.zip` → `manager-bin`, `webui/index.html`, `RGSX.sh`
  - `RGSX_update_windows_latest.zip` → `ports/RGSX/manager-bin.exe`, `webui/`,
    `windows/RGSX rust.bat`
  - `RGSX_full_latest.zip` → `ports/` + `windows/`
- Windows paketi RetroBat test kurulumunda `RGSX rust.bat` ile başlar (WebUI :5000).
- Linux binary Batocera hedefinde çalışır (TVUI + WebUI canlı smoke).
- Doğrulama bitince test tag'i ve draft release temizlenir; bu dosya done/'a taşınır.

---

## İlerleme

- 2026-08-26 — Görev oluşturuldu (gap-02 bitiş notundan ayrıştırıldı).
- 2026-08-26 — Tamamlandı. Test tag'i `v9.9.9-test` **aydemir fork**'una push edildi
  (origin'e değil — gerçek release/Discord tetiklenmesin); 3 koşu sonucu yeşile ulaştı
  (run 32966068702, ~7 dk). Bulunan ve giderilen tahmin hataları:
  1. `cargo build` repo kökünde çalışıyordu → workspace `manager-rs/` altında;
     build adımlarına `working-directory: manager-rs` + artifact yolları düzeltildi.
  2. Repo'daki kişisel `manager-rs/.cargo/config.toml`
     (`target-dir = C:/Users/lv/RGSX/rust-target`) Linux'ta `LD_LIBRARY_PATH` join
     panic'ine yol açıyordu → repodan çıkarıldı + `.gitignore`'a eklendi (yerel dosya
     duruyor; CI default `manager-rs/target` kullanıyor).
  3. Runner CMake 4.x vs sdl2-sys `cmake_minimum_required(<3.5)` → top-level env
     `CMAKE_POLICY_VERSION_MINIMUM: "3.5"`.
  4. `upload-artifact@v4` exec bit taşımıyor → zip öncesi `chmod +x manager-bin RGSX.sh`
     (create_release); `RGSX.sh` update zip'e eklendi; git'te 100644→100755.
  5. Fork'ta `DISCORD_WEBHOOK` secret yok → adım secret yoksa atlanır (origin davranışı aynı).
  Doğrulama: 3 zip indirildi; içerik checklist'e uygun (`update`: manager-bin+webui+RGSX.sh;
  `windows`: ports/RGSX/manager-bin.exe+webui+windows; `full`: ports+windows);
  `manager-bin`/`RGSX.sh` `-rwxr-xr-x`; WSL ldd: glibc bağımlılıklarının tamamı çözülüyor
  (not found=0). Test release + tag silindi. Kalan (cihaz tarafı): Batocera'da canlı smoke +
  RetroBat'ta `RGSX rust.bat` boot testi — kullanıcı doğrulaması.
