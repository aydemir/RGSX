# TASK-009 — Katalog isteklerinin hâlâ Python'a proxy'lenip proxy'lenmediğinin tespiti

- **id:** TASK-009
- **title:** Dağıtılan manager-bin'in katalog yanıtlarını saf-native verip vermediğinin tespiti
- **status:** done
- **priority:** P2
- **created:** 2026-08-18
- **environment:** both
- **tags:** catalog, proxy, native, python-free, detection

## Kaynak

- `api.rs` içinde `if !intercept_locally { if let Some(c) = &state.catalog { c.post_json(...) } }` Python proxy dalı vardı. Dağıtılan ortamda `state.catalog`'un ne olduğunun (NativeCatalog mı PythonCatalog mı) ve katalog uçlarının native mi proxy mi düştüğünün doğrulanması gerekiyordu.

## Açıklama

Dağıtılan `manager-bin` katalog uçlarını (`/api/platforms`, `/api/games`, `/api/translations`, `/api/image`) saf-native mi yoksa Python katalog örneğine mi proxy'liyor, tespit et.

## Kapsam / Dosyalar

- `manager-rs/manager-bin/src/main.rs:254-266` — `native_catalog = RGSX_NATIVE_CATALOG` (varsayılan **true**); true ise `catalog = Some(NativeCatalog::from_env())`, false ise yalnız `RGSX_PYTHON_MANAGER_URL` set ise `PythonCatalog`. Test ortamında her ikisi de set değil → **NativeCatalog**.
- `manager-rs/manager-http/src/api.rs` — katalog proxy dalı `if let Some(c) = &state.catalog` → `c` burada NativeCatalog.
- `manager-rs/manager-http/src/catalog.rs:530` — `build_translations` `languages_folder/{lang}.json` okur.

## Doğrulama (runtime)

1. **Env:** Çalışan `manager-bin` ortamında `RGSX_PYTHON_MANAGER_URL` ve `RGSX_NATIVE_CATALOG` SET DEĞİL → `native_catalog=true` → `state.catalog = Some(NativeCatalog)` (PythonCatalog ASLA oluşmadı).
2. **Startup:** `disk taraması: 2 platformda 3 kurulu oyun bulundu` satırı `NativeCatalog.installed_list()`'ten gelir (main.rs:272) → native catalog aktif.
3. **`/api/platforms`:** `{"count":152,"platforms":[...]}` — `systems_list.json`'dan native yüklendi (Python proxy olsa farklı shape/forward olurdu).
4. **`/api/games/3do`:** `{"count":0,"games":[],"platform":"3do","success":true}` — native-shaped yanıt (boş içerik = veri dosyası deployment'ta yok, proxy DEĞİL).
5. **`/api/translations?lang=fr`:** `{"language":"fr","success":true,"translations":{"_language":"fr"}}` — native `build_translations` shape; dil dosyası bulunamadığı için seyrek (bkz. Gözlem).
6. **Log:** Katalog istekleri sırasında "proxy"/python/catalog-forward referansı YOK; yalnız torrent/librqbit ve disk taraması satırları.

## Sonuç

Katalog uçları **saf-native (NativeCatalog)** tarafından veriliyor; Python'a hiçbir proxy yok. `state.catalog` "Some" ama içi Rust `NativeCatalog` — Python proxy dalı runtime'da tamamen inaktif.

## Gözlem (native tarafta config boşluğu — proxy DEĞİL)

- Binary CWD = `/root/RGSX`; `languages_folder` = `RGSX_LANGUAGES_FOLDER` veya `RGSX_DATA_DIR/languages` (ikisi de set değil) → `/root/RGSX/languages` aranıyor; gerçek dil dosyaları `ports/RGSX/languages/`'ta. Bu yüzden `/api/translations` seyrek.
- `/test/roms/ports/RGSX/languages` ve `games/*.json` deployment'a kopyalanmamış → `/api/games` boş.
- Bu, UI dilini ETKİLEMEZ (webui string'leri bundle `i18n.strings.js`'ten gelir, Playwright ile 5 dil PASS). Yalnızca **katalog veri dili** etkilenir.
- Olası takip: `ports/RGSX/{languages,games,systems_list.json}`'ı binary data dir'ine deploy et ve `RGSX_LANGUAGES_FOLDER`/`RGSX_DATA_DIR` set et (server restart gerekir). Ayrı görev olarak açılabilir.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu.
- 2026-08-18 — statik + canlı kanıt: NativeCatalog aktif, Python proxy yok; native veri-dosyası eksikliği gözlemlendi.
