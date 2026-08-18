# TASK-006 — Wii torrent indirmesinin hedefe uçtan uca tamamlanma doğrulaması

- **id:** TASK-006
- **title:** Wii (Torrent) indirmesinin /test/roms/wii/ altına düşmesinin kanıtlanması
- **status:** todo
- **priority:** P1
- **created:** 2026-08-18
- **environment:** linux
- **tags:** torrent, librqbit, wii, verification, link_or_copy

## Kaynak

- `d97de92` ile torrent düzeltmesi yapıldı ve canlı testte ~430MB'a kadar indi (doğru tek dosya seçimi, hata yok). Ancak indirme yavaş peer nedeniyle yarım kaldı ve **dosyanın `link_or_copy` ile hedefe (`/test/roms/wii/`) düştüğü hiçbir zaman doğrulanmadı**. History durumu `COMPLETED` ve hedef dosya varlığı teyit edilmedi.

## Açıklama

Torrent düzeltmesinin gerçekten uçtan uca çalıştığını (indir → doğrula → hedefe kopyala/symlink) kanıtla. `manager-torrent/src/lib.rs` zaten düzeltildi; bu görev **yalnız doğrulama** (gerekirse daha küçük/tek-dosyalı bir torrent ile tamamlanma hızlı test edilebilir).

## Kapsam / Dosyalar

- Doğrulama scripti: `/tmp/opencode/trig_torrent.py` (mevcut) + polling.
- İlgili kod: `manager-rs/manager-torrent/src/lib.rs` (değişmeyecek, referans).
- Log: `/test/roms/ports/RGSX/manager-bin.log`.

## Doğrulama

- Wii torrent tetiklenir; `GET /api/history` polling ile `entity_state == COMPLETED` olana kadar izlenir.
- İndirilen dosya `/test/roms/wii/<oyun>.zip` olarak mevcut mu kontrol edilir.
- Log'da `error`/`fail` yok; `link_or_copy` / symlink adımı görünür.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu (torrent fix sonrası yarım kalan uçtan uca doğrulama).
