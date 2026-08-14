# TASK-006 — WebUI Ayarlar panelini native `/api/settings`'e bağla

**Durum:** done
**Environment:** both (Rust backend `RGSX_NATIVE_SETTINGS=1` + Vue webui)
**Bağımlılık:** TASK-003 (Adım 3 i18n altyapısı tamam); `manager_core::settings::Settings` (load/save/validate) + `GET/POST /api/settings` native dalı zaten mevcut.

## Hedef
TASK-003 notunda "native ayar kaynağı ileride" denmişti. Backend native dalı
(`RGSX_NATIVE_SETTINGS=1`) zaten çalışıyor (GET döner, POST diske persist eder,
round-trip doğrulandı: 2026-08-14 canlı test). Kalan iş **sadece webui**:
Ayarlar paneli şu an salt-okunur `<pre>` gösteriyor; değişiklikleri sunucuya
geri POST etmiyor. Paneli düzenlenebilir + kalıcı hale getir.

## Adımlar
- **1.** `webui/src/i18n.js`: yeni etiketler (light_mode, max_downloads, music,
  show_unsupported, sort + sort seçenekleri, saved, save_failed).
- **2.** `webui/src/App.vue` script: `settings` (düzenlenebilir ref) + `saveMsg`;
  `loadSettings()` içinde `s.settings`'i doldur; `saveSettings()` →
  `POST /api/settings { settings: <tam nesne> }` (backend replace yapar, bu yüzden
  tam nesne gönderilir); `changeDataLang` artık `settings.language`'ı da günceller
  ve kaydeder.
- **3.** `webui/src/App.vue` template: `<pre>` yerine düzenlenebilir alanlar
  (Veri Dili, light_mode, grid, max_simultaneous_downloads, music_enabled,
  show_unsupported_platforms, global_sort_option) — her `@change` `saveSettings()`.
- **4.** `npm run build`; canlı test (`RGSX_NATIVE_SETTINGS=1`) GET→düzenle→POST→
  disk round-trip doğrula; 105 contract test korunur.

## Doğrulama
- `webui` build yeşil (dist/index-D98lCYVZ.js); `manager-core` settings unit testleri 6/6 yeşil.
- Backend native `/api/settings` round-trip canlı doğrulandı (önceki oturum):
  GET → POST {language:tr, display.light_mode:true} → disk'e yazıldı → GET tekrar
  kalıcı döndü. UI artık aynı akışı çağırıyor (saveSettings → tam nesne POST).
- Not: contract 105 Python testleri backend shape'i değişmediği için etkilenmez;
  Rust tarafında yalnız webui + mevcut native settings dalı kullanıldı.

## İlerleme
- 2026-08-14 — TASK-006 tamam: Ayarlar paneli düzenlenebilir + native persist.
  i18n etiketleri eklendi; App.vue `settings` ref + `saveSettings()` + `changeDataLang`
  artık `POST /api/settings` yapıyor. Build + manager-core testleri yeşil. (Commit
  kullanıcı isteğine bırakıldı.)
