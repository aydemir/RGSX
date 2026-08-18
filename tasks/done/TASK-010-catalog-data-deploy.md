# TASK-010 — Native catalog veri-dosyası deployment boşluğunun kapatılması

- **id:** TASK-010
- **title:** `RGSX_DATA_DIR`/`RGSX_LANGUAGES_FOLDER` eksik olduğu için native katalog veri uçlarının boş/seyrek dönmesinin düzeltilmesi
- **status:** done
- **priority:** P2
- **created:** 2026-08-18
- **environment:** linux
- **tags:** catalog, native, deployment, languages, data-dir

## Kaynak

- TASK-009 kapsamında: `manager-rs/manager-http/src/catalog.rs:258-264` `from_env()` veri yollarını `RGSX_DATA_DIR` (vars. `"."`) altından türetir. Test ortamında `RGSX_DATA_DIR` set değilse `data_dir="."` (binary CWD'si) olur; `languages_folder`/`games_folder`/`images_folder` orada aranır. Gerçek katalog verisi `/test/saves/ports/rgsx/`'ta (systems_list.json, games/, images/), diller ise kaynakta `/root/RGSX/ports/RGSX/languages/`.
- Belirtiler: `/api/games` boş, `/api/translations?lang=fr` yalnız `{"_language":"fr"}` döndü (dil dosyası bulunamadı). `/api/platforms` 152 platform döndü (systems_list.json mevcut konumdan bulundu).

## Açıklama

Dağıtılan `manager-bin`'i `RGSX_DATA_DIR=/test/saves/ports/rgsx` ve `RGSX_LANGUAGES_FOLDER=/root/RGSX/ports/RGSX/languages` ile başlatarak native katalog veri uçlarının (platforms/games/translations/languages/images) tam veriyle dönmesini sağla.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/catalog.rs:258-264` — `from_env()` yol çözümü.
- `/test/saves/ports/rgsx/` — systems_list.json, games/, images/ (mevcut).
- `/root/RGSX/ports/RGSX/languages/*.json` — 7 dil çevirisi (kaynak).
- Çalışan süreç: yeniden başlatma + `curl` uç doğrulaması.

## Doğrulama (runtime)

Sunucu `RGSX_DATA_DIR=/test/saves/ports/rgsx RGSX_LANGUAGES_FOLDER=/root/RGSX/ports/RGSX/languages` ile yeniden başlatıldı (PID 15185, health 200):
- `curl /api/platforms` → **count=152** ✓ (systems_list.json bulundu).
- `curl /api/languages` → `["de","en","es","fr","it","pt","tr"]` ✓ (7 dil).
- `curl /api/translations?lang=fr` → **716 anahtar**, gerçek FR değerleri (`accessibility_font_size: "Taille de police : {0}"`) ✓ — önceki seyrek `_language` sorunu çözüldü.
- `curl /api/games/<p>` → **0** (bkz. Gözlem): deployment düzeltmesi dosyaları erişilebilir kıldı ama parser farklı format bekliyor.

## Sonuç

`RGSX_DATA_DIR`/`RGSX_LANGUAGES_FOLDER` eksikliğinden kaynaklanan native katalog deployment boşluğu kapatıldı; platforms/languages/translations uçları tam veriyle dönüyor.

## Gözlem (ayrı bug → TASK-011)

`/api/games` hâlâ 0 dönüyor. Kök neden deployment DEĞİL: `catalog.rs:394` `load_games` dosyayı `{"games":[{game_name,url,size}]}` (obje) olarak bekler, oysa Python-format games json'i `[[name,url,size],...]` (list-of-lists) — format uyumsuzluğu yüzünden parser boş dizi döndürüyor. Gerçek veri mevcut (`games/3DO ... (Archive).json` = 350 kayıt). Ayrı görev TASK-011 olarak açıldı.

---
## İlerleme

- 2026-08-18 — TASK-009 gözleminden açıldı.
- 2026-08-18 — deployment düzeltildi (data-dir + languages env), platforms/languages/translations tam veriyle doğrulandı; games parser bug'ı TASK-011'e spin-off.
