# TASK-002s — Faz 12f: Settings şeması native port (Rust typed schema + validasyon)

- **id:** TASK-002s
- **title:** Faz 12f — `rgsx_settings.py`/`config.py` ayar şemasını Rust'e taşı (typed `Settings` struct + serde + validasyon + load/save)
- **status:** done
- **priority:** P1
- **created:** 2026-08-13
- **environment:** both
- **tags:** rust, faz-12, settings, göç, şema, validasyon
- **parent:** TASK-002 / ROADMAP_FAZ12 (ertelenmiş kalem: "Settings şeması")

## Kaynak

- `docs/roadmap/ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` — §3 #10 "Settings kalıcılığı (`rgsx_settings.py`, `config.py`) — yalnız okuma/yazma değil, şema + validasyon sözleşmesi port edilmeli." + §1 bağımlılık tablosu (12c/12d `dirs`/`directories`).
- Keşif (codegraph, 2026-08-13): `ports/RGSX/rgsx_settings.py` (`load_rgsx_settings`/`save_rgsx_settings`, `default_settings`), `ports/RGSX/config.py` (`RGSX_SETTINGS_PATH = SAVE_FOLDER/rgsx_settings.json`, ayrı API key dosyaları `1FichierAPI.txt` vb.), `manager-rs/manager-http/src/state.rs` (`StateData.settings: serde_json::Value` — şu an placeholder), `manager-rs/manager-http/src/api.rs` (`settings_get`/`settings_post`/`save_filters` handler'ları).

## Kök neden / Davranış kuralları (araştırma)

**Mevcut durum:** Rust `StateData.settings` ham `serde_json::Value` ve `settings_post` onu doğrudan klonlayıp yazıyor — **hiçbir şema/validasyon yok**. Python `rgsx_settings.py` ise:
- `default_settings` sözlüğü ile birleşim (eksik key'leri varsayılana tamamlar),
- `language` key'ini **kasıtlı atlar** (Faz 11: "key yok = kullanıcı seçimi yok" sözleşmesi — dosyaya default enjekte edilmez),
- `auto_extract` ayrı dosyada (`get_auto_extract`/`set_auto_extract`),
- `api_keys` (1fichier/alldebrid/debridlink/realdebrid/torbox) ayrı `.txt` dosyalarında (`load_api_keys`/`save_api_keys`),
- `web_service_at_boot`/`custom_dns_at_boot` Linux-only systemd toggle (`toggle_web_service_at_boot`),
- `game_filters` (region_filters vb.) ayrı bir alt nesne.

**Davranış kuralları (göç sırasında değişmez):**
1. Settings sözleşmesi `tests/test_api_contract.py::TestSettings` (settings_get / settings_post / missing_param) + `manager-rs/manager-http/tests/contract.rs` ile yeşil kalır.
2. `GET /api/settings` hâlâ `{success, settings, system_info}` döner; `settings` Python'daki `default_settings` birleşimiyle uyumlu olur.
3. `language` için "yok = seçim yok" kuralı korunur (Rust struct'ta `Option<String>` / veya load'da atlama).
4. Kesintisiz göç: gerçek persist henüz native değilse `RGSX_NATIVE_SETTINGS=1` flag'iyle açılır; flag kapalı → mevcut Python proxy / placeholder davranışı (risk sıfır).

## Kapsam / Dosyalar

- **Yeni:** `manager-rs/manager-core/src/settings.rs` — `Settings` struct (tüm `default_settings` alanları typed: language `Option<String>`, accessibility, display, symlink, sources, bayraklar, platform_custom_paths `HashMap`, max_simultaneous_downloads), `Default` impl, `load()`/`save()` (`dirs`/`SAVE_FOLDER` env'den `rgsx_settings.json`), validasyon (ör. `grid` izin verilen değerler, `font_scale > 0`, `max_simultaneous_downloads` 1..=N).
- **Değişen:** `manager-rs/manager-http/src/state.rs` — `StateData.settings` tipi `manager_core::settings::Settings`'e yükseltilir (backward: handler'lar `serde_json::to_value` ile uyumlu kalır).
- **Değişen:** `manager-rs/manager-http/src/api.rs` — `settings_get` native `Settings::load()` + `system_info` (env'den `SAVE_FOLDER`/`ROMS_FOLDER`/platform sayısı) döndürür; `settings_post` native validasyon + `save()` yapar, `language`/auto_extract/api_keys/linux-toggle alanlarını flag'e göre ya native ya da Python proxy'sine bırakır.
- **Test:** `manager-rs/manager-http/tests/contract.rs` ayar sözleşmesi korunur; yeni `manager-core/src/settings.rs` birim testleri (defaults birleşimi, validasyon reddi, language atlama).
- **Belge:** `docs/PROJECT_MAP.md` + `docs/roadmap/ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` güncellenir; `tasks/done/`'a taşınır.

## Doğrulama

- `cargo build` (workspace) yeşil.
- `cargo test -p manager-core` (settings birim testleri) yeşil.
- `cargo test -p manager-http` contract (102 test) yeşil — settings_get/post sözleşmesi bozulmaz.
- `RGSX_NATIVE_SETTINGS=0` (varsayılan) → davranış değişmez (Python proxy/placeholder); `=1` → native load/save + validasyon aktif, canlı smoke: settings GET/POST sonra dosya yazımı doğrulanır.

---

## İlerleme

- 2026-08-13 — başlandı (görev dosyası oluşturuldu, codegraph ile mevcut kod doğrulandı, plan sunuldu; implementasyon onay bekliyor).
- 2026-08-13 — tamamlandı: `manager-core/src/settings.rs` (typed `Settings` + `Default` + `load/save/validate` + `system_info`), `manager-http/src/api.rs` `settings_get`/`settings_post` native yola bağlandı (`RGSX_NATIVE_SETTINGS=1`). Birim testler (settings.rs 6 test) + contract `test_settings_native_roundtrip` yeşil; tam contract 103/103. Option A (temel): yan-etkili alanlar (auto_extract/api_keys/linux-toggle) native save'de strip edilir, port sonraki faza bırakıldı.
