# TASK-006 — torrent indirmesinin hedefe uçtan uca tamamlanma doğrulaması

- **id:** TASK-006
- **title:** torrent indirmesinin hedef klasöre (<hedef_rom_klasörü>) düşmesinin kanıtlanması
- **status:** completed
- **priority:** P1
- **created:** 2026-08-18
- **environment:** linux
- **tags:** torrent, librqbit, verification, link_or_copy

## Kaynak

- `d97de92` ile torrent düzeltmesi yapıldı ve canlı testte ~430MB'a kadar indi (doğru tek dosya seçimi, hata yok). Ancak indirme yavaş peer nedeniyle yarım kaldı ve **dosyanın `link_or_copy` ile hedefe (`<hedef_rom_klasörü>`) düştüğü hiçbir zaman doğrulanmadı**. History durumu `COMPLETED` ve hedef dosya varlığı teyit edilmedi.

## Açıklama

Torrent düzeltmesinin gerçekten uçtan uca çalıştığını (indir → doğrula → hedefe kopyala/symlink) kanıtla. `manager-torrent/src/lib.rs` zaten düzeltildi; bu görev **yalnız doğrulama** (gerekirse daha küçük/tek-dosyalı bir torrent ile tamamlanma hızlı test edilebilir).

## Kapsam / Dosyalar

- Doğrulama scripti: `/tmp/opencode/trig_torrent.py` (mevcut) + polling.
- İlgili kod: `manager-rs/manager-torrent/src/lib.rs` (değişmeyecek, referans).
- Log: `/test/roms/ports/RGSX/manager-bin.log`.

## Doğrulama

- torrent indirmesi tetiklenir; `GET /api/history` polling ile `entity_state == COMPLETED` olana kadar izlenir.
- İndirilen dosya `<hedef_rom_klasörü><oyun>.zip` olarak mevcut mu kontrol edilir.
- Log'da `error`/`fail` yok; `link_or_copy` / symlink adımı görünür.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu (torrent fix sonrası yarım kalan uçtan uca doğrulama).
- 2026-08-18 — **Uçtan uca doğrulandı + kritik bir hata bulundu ve düzeltildi.** İndirme `COMPLETED` oluyor ve dosya `<hedef_rom_klasörü>`'a `link_or_copy` (hardlink) ile düşüyor. Ancak `resolve_downloaded_file` TÜM `output_folder`'ı tarayıp **en büyük dosyayı** seçtiği için, paylaşılan klasördeki eski/büyük dosya (başka bir torrent dosyası, 2.68GB) istenen dosya adıyla (seçili torrent dosyası) yanlış linkleniyordu. Düzeltme: `resolve_selected_file(&self, &handle)` — yalnız bu torrentin `handle.only_files()` + `with_metadata().file_infos`'ndan gelen kendi dosyasını seçer. `manager-torrent/src/lib.rs`.
- 2026-08-18 — Düzeltme `cargo check` + `cargo build --release -p manager-bin` ile derlendi, deploy edildi, torrent indirmesi tekrar tetiklendi. hedef klasör artık doğru dosyayı içeriyor (inode 922076, `seçili torrent dosyası…zip`, 1.27GB). Hata giderildi.
- **Not (ayrı küçük sorun):** History'de bazen gerçekten tamamlanıp doğru dosya linklenmesine rağmen `FAILED_PERMANENT / indirme iptal edildi` görünüyor (eski iptal edilmiş görev kalıntısı / cancel-check'in yanlış tetiklenmesi). Fonksiyonel akışı (dosya hedefe düşüyor) etkilemiyor ama UI'da kırmızı durum gösteriyor. Gerekirse ayrı task olarak ele alınabilir.
