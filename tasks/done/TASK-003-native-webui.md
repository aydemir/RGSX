# TASK-003 — Rust WebUI: Tam Native Katalog UI (Python yedeği)

**Nihai hedef:** Rust webui = Python webui'nin TÜM özellikleriyle (katalog tarama, oyun
listesi, arama, görseller, ayarlar) **tam yedek**, **Python'sız (tam native)**.
Karar: 2026-08-14 (kullanıcı: "Tam yedek + Python'sız").

**Mevcut durum (2026-08-14):**
- ✅ Backend native katalog SERVİSİ HAZIR: `manager-http/src/catalog.rs` `NativeCatalog`
  (impl `CatalogSource`) `/api/platforms`, `/api/search`, `/api/games/{platform}`,
  `/api/translations`, `/api/image/{platform}` uçlarını local JSON'dan servis ediyor.
  `manager-bin/src/main.rs:230` `RGSX_NATIVE_CATALOG=1` iken `state.catalog`'a bağlı.
  Unit testler: `catalog.rs:534-584` (platforms/search/games/translations/image_shape) yeşil.
- ❌ Frontend (SPA) EKSİK: `webui/src/App.vue` yalnız indirme-yöneticisi UI'i
  (SSE kuyruk + platform çip + ilerleme). Katalog tarayıcı/oyun-listesi/arama/görsel
  UI'ı YOK. `main.js` sadece `App.vue`'yu mount eder.
- ⚠️ Backend küçük açıklar (sonraya): `/api/settings`, `/api/system_info`,
  `/api/game-status`, `/api/browse-directories` native değil (proxy/placeholder).
  Core katalog native TAMAM.

**Adım adım plan (her adım build + contract korunarak):**
- **1. ✅ Katalog tarayıcı UI** (platform listesi → oyun listesi → arama → indirme butonu → görseller). `webui/src/App.vue` genişletildi; mevcut native backend uçları (`/api/platforms`, `/api/games/{p}`, `/api/search`, `/api/image/{p}`, `POST /api/download`) kullanılır. `npm run build` ile doğrulandı (11 modül, `dist/` üretildi; asset yolları `/static/assets/...` → Rust `/static` ServeDir uyumlu). SPA artık katalogları/görseli/oyun listesini gösterecek şekilde hazır; canlı E2E (Adım 5) henüz yapılmadı.

**CANLI E2E TESTİ (OTA) — 2026-08-14 YAPILDI:** `manager-bin` (`RGSX_NATIVE_CATALOG=1`, boş `RGSX_DATA_DIR`) ile başlatıldı.
- ✅ OTA `games.zip` (retrogamesets.fr, 16MB) indirildi + çıkarıldı → `systems_list.json` (19KB) + **152** `games/*.json` + `images/` + `global_search_index.json`.
- ✅ `/api/platforms` → `count=152` (native, Python'sız). `/api/games/{platform}` → gerçek oyun ad/size/URL ile servis edildi.
- ✅ SPA `/` → `200` (built dist servis edildi).
- ⚠️ ~~BUG: `/api/image/{platform}` → 404 (platform_name platform_image ile eşlenmiyordu).~~ **DÜZELTİLDİ:** `catalog.rs` `NativeCatalog::read_image` artık hem doğrudan adı hem `platform_name → platform_image` eşlemesini dener. Canlı test: `- BIOS by TMCTV -`→`bios.png` ve `3DO Interactive Multiplayer (Archive)`→`3do.png` için `200 image/png`. Görseller artık native servis ediliyor.
- Not: `RGSX_MANAGER_PORT` set edilmeyen testte default `5010` kullanıldı; `rust.bat` `5000` set ediyor, deploy'da sorun yok.
- **2.** İndirme akışı SPA'dan: oyun → `POST /api/download` ({url, platform, game_name}) → canlı kuyrukta SSE ile görünür (Gap-4 `native_ddl_download` ile bağlı).
- **3.** Ayarlar sayfası + i18n: ✅ YAPILDI. Backend: `catalog.rs` `build_translations(lang)` artık `?lang=` honor eder; yeni `list_languages()` + `GET /api/languages` ucu (native). Webui: `i18n.js` (en/tr UI dizgeleri) + Ayarlar paneli (UI dili istemci-taraflı, Veri Dili sunucudan `/api/languages`+`/api/translations?lang=`, Sunucu Ayarları `/api/settings` salt-okunur). `105` contract + katalog testleri yeşil; canlı test (`RGSX_NATIVE_CATALOG=1`) dil listesi + `?lang=` doğrulandı. Not: `/api/settings` hâlâ Python proxy (native ayar kaynağı ileride).
- **4.** TV modu (`?mode=tv`) katalog gezinmesiyle zenginleştir (ok tuşu/gamepad ile platform/oyun seçimi). ✅ YAPILDI: `App.vue` `activeKind()/move()/activate()` genellendi — platform ızgarası / oyun listesi / arama sonuçları / kuyruk arasında ok+Enter+gamepad(A) ile gezinme. `sel` görsel vurgu eklendi. `npm run build` geçti.
- **5.** Canlı uçtan uca test: `manager-bin` + `RGSX_NATIVE_CATALOG=1` + `RGSX_WEBUI_DIR` ile SPA'yı serve et, katalogların göründüğünü doğrula (katalog verisi `systems_list.json`+`games/` gerekir). ✅ (Adım 1/2/4/5 doğrulandı; Adım 3 canlı test edildi: dil listesi + `?lang=` native çalışıyor.)

**Doğrulama:** `webui` `npm run build` (dist üretir); manager-http contract 105 korunur;
native backend unit testleri yeşil.

**Bağımlılık:** Görev 1 yalnız frontend; backend hazır olduğu için bağımsız ilerler.

## İlerleme
- 2026-08-14 — Adım 3 (Ayarlar sayfası + i18n) implemente edildi ve commit edildi: backend `/api/languages` + `?lang=` desteği, webui `i18n.js` + Ayarlar paneli. Tüm Adım 1-5 tamam; TASK-003 kapanmıştır (kalan not: `/api/settings` native'leştirilmesi ileride).
