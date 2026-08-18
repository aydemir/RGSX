# TASK-002-gap-17 — Settings Şema Parity (eksik ayar alanları)

- **id:** TASK-002-gap-17
- **title:** Settings schema parity (background_theme, web_service_at_boot persist, gamelist update days, app_version, app/config dir model)
- **status:** done
- **priority:** P3
- **created:** 2026-08-15
- **environment:** both
- **tags:** settings, config, parity
- **parent:** TASK-002

## Karar (2026-08-15)

`/api/settings` **request contract değişmez.** Ancak `manager-core/src/settings.rs` şemasında
Python'da olan bazı ayarlar EKSİK veya persist EDİLMİYOR; round-trip'de veri kaybı var.

> BELİRSİZ: `RGSX_APP_DIR`/`RGSX_CONFIG_DIR` path modeli Rust'ta `RGSX_DATA_DIR`+`RGSX_SETTINGS_PATH`
> ile değiştirildi — bu bilinçli bir fark mı yoksa parity açığı mı? Docker mod yol çözümü
> farklı olduğundan BELİRSİZ; kullanıcı onayı gerekir.

## Python Referans Davranışı

- `ports/RGSX/ports/RGSX/rgsx_settings.py:499-519` `display.background_theme`
- `ports/RGSX/ports/RGSX/rgsx_settings.py:85` `web_service_at_boot` (persist edilir)
- `ports/RGSX/ports/RGSX/config.py:42` `GAMELIST_UPDATE_DAYS`
- `ports/RGSX/ports/RGSX/config.py:30` `app_version`
- `ports/RGSX/ports/RGSX/config.py:62-63` `RGSX_APP_DIR` / `RGSX_CONFIG_DIR`

## Rust Mevcut Durum (❌ / ⚠️)

- `manager-core/src/settings.rs:45-57` `Display` struct'ta `background_theme` alanı YOK → round-trip veri kaybı (❌).
- `manager-core/src/settings.rs:103` `web_service_at_boot` alanı var ama `save()` `:231` siler → persist edilmiyor (⚠️).
- `manager-core/src/settings.rs` şemasında `gamelist_update_days` ayarı YOK (❌).
- `manager-core/src/settings.rs` şemasında `app_version` alanı YOK (❌).
- Rust tek `RGSX_DATA_DIR`+`RGSX_SETTINGS_PATH` kullanır; `RGSX_APP_DIR`/`RGSX_CONFIG_DIR` ayrımı YOK (⚠️, path model farkı).

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-rs/manager-core/src/settings.rs` — `Display.background_theme`, `gamelist_update_days`,
  `app_version` alanları + `web_service_at_boot` persist (`:231` silme kaldırılır)
- `manager-rs/manager-bin/src/main.rs` — env path modeli (gerekirse)

## Bağımlılık

- Yok (bağımsız, düşük öncelik). `TASK-006-native-settings-webui` ile örtüşebilir.

## Doğrulama

- `rgsx_settings.py` ayarları Rust `settings.rs`'e round-trip'de kaybolmaz (test: `tests.rs` settings round-trip).
- `web_service_at_boot` diske yazılır ve geri okunur.

## 2026-08-18 BAYATLIK AUDIT + UYGULAMA

`manager-core/src/settings.rs` ve Python referansları (`rgsx_settings.py`, `config.py`) karşılaştırıldı.
5 maddeden **yalnız 1'i gerçek parity açığı**, diğerleri sabit veya kasıtlı tasarım farkı:

| Madde | Durum | Karar |
|---|---|---|
| `Display.background_theme` | ❌ GERÇEK AÇIK | **Uygulandı**: `Display.background_theme: String` eklendi (default `"default"`, izin kümesi `default/sunset/forest/midnight`). `load()` geçersiz değeri `"default"`'a düşürür (Python `get_display_background_theme` parity). Round-trip + coercion testleri eklendi. |
| `web_service_at_boot` persist | ⚠️ KASITLI FARK | `save()` bunu kasıtlı siler (settings.rs:9-10 yorumu: systemd ayrı mekanizma, native save'de yazılmaz). Python parity'si ama Rust native modelinde **bilinçli**; değiştirilmedi. |
| `gamelist_update_days` | ❌ YANLIŞ AÇIK | `config.py:42 GAMELIST_UPDATE_DAYS = 1` bir **sabit**, kullanıcı ayarı değil. `settings.rs`'e eklenmedi. |
| `app_version` | ❌ YANLIŞ AÇIK | `config.py:30 app_version = "2.6.5.7"` build **sabit** (versiyon gösterimi/update kontrolü). Ayar alanı değil; eklenmedi. |
| Path model (RGSX_APP_DIR/CONFIG_DIR) | ⚠️ KASITLI FARK | Rust tek `RGSX_DATA_DIR`+`RGSX_SETTINGS_PATH`'e konsolide (doc zaten "belirsiz" demişti). Bilinçli fark; değiştirilmedi. |

**Not:** `background_theme` Rust şemasına eklendi ama webui (App.vue) henüz bu alanı gösterip
yazmıyor — tam UI parity `TASK-006-native-settings-webui` kapsamında (bu görev yalnızca
Rust şema parity'sini kapatır).
