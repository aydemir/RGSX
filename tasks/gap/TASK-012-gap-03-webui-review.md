# TASK-012-gap-03 — WebUI (Vue SPA) Python→Rust geçiş incelemesi: kaçırılan noktalar

- **id:** TASK-012-gap-03
- **title:** WebUI parite taraması — kaybolan TV modu, kukla update-cache, ayarlar sekmesi sapması ve davranış hataları
- **status:** todo
- **priority:** P1
- **created:** 2026-08-22
- **environment:** both
- **tags:** webui, vue3, parity, review, sse, settings, tv-mode

## Kaynak

- Kullanıcı isteği (2026-08-22): "webui tarafında rust'a geçerken kaçırılan noktaları bir gözden
  geçir". `App.vue` (1315 satır) + `api.js` + Rust route tablosu (`manager-http/src/lib.rs:40-78`)
  + Python WebUI referansı (`ports/RGSX/static/js/app.js` 2844 satır, `static/index.html`) karşı-
  laştırıldı. gap-01 metodolojisiyle: her iddia dosya:satır kanıtlı.

## Bulgular

### Kritik
1. **TV modu + gamepad: native karar sonrası resmen emekli edilmemiş miras** — SPA `?mode=tv`
   gamepad kodu Faz 12/13 döneminde eklendi (`b5827d2`, 2026-08-13, "WebUI+TVUI tek SPA"
   stratejisi); `cd6a22d` (2026-08-15, "tab'lı UI") refactor'unda commit mesajında hiç anılmadan
   çıktı; ardından native SDL2 TVUI kilitlendi (yön B, TASK-012g). Kod bugün App.vue'da YOK;
   ama yön-B aksiyonu eksik kaldı: `rgsx-webui-spa` skill'i hâlâ `?mode=tv`/gamepad anlatıyor,
   `/api/es-input` HTTP ucu (`lib.rs:54`) TÜKETİCİSİZ duruyor ve contract testine sabitlenmiş
   (`contract.rs:210 test_es_input_shape`), `manager-bin/src/main.rs:362` kiosk yorumları bayat.
   **KARAR (kullanıcı onaylı, Seçenek A): `?mode=tv` resmen emekliye ayrılır** — tek TVUI =
   native SDL2. (Yön B seçildiği anda bu aksiyonun plana alınması gerekirdi.)
2. **🔄 (katalog yenile) butonu kukla — çift hata** — SPA `POST /api/update-cache` çağırır
   (App.vue:225), route `get()` kayıtlı (`lib.rs:56`) → her tıkta 405, sessizce yutulur; ardından
   KOŞULSUZ "katalog yenilendi" başarı toast'u (App.vue:230). Üstüne endpoint zaten placeholder
   (`{deleted:0}`, api.rs:413) — Python'daki gamelist güncelleme akışının Rust karşılığı hiç
   yazılmadı.
3. **Self-update'in WebUI ayağı yok** — uçlar hazır (`/api/manager-update*`, lib.rs:57-58),
   TVUI banner'lı (TASK-012m); WebUI'de hiç iz yok. WebUI kullanıcısı manager güncellemesinden
   habersiz.

### Orta
4. **`mode: 'now'|'queue'` yok sayılıyor** — App.vue:459 gönderiyor; api.rs'te `"mode"` yalnız
   1 yerde geçiyor (yanıt üretimi). ⬇️(hemen) ile ➕(kuyruk) aynı davranıyor; Python paritesi kırık.
5. **i18n bypass** — boot ekranı komple hardcoded Türkçe ("Katalog hazırlanamadı" App.vue:193,
   "Tekrar dene"/"Çevrimdışı devam" 737-738, "Çevrimdışı mod" 747); Region-priority modalı komple
   İngilizce (1068-1081); "Region priority" butonu 818. i18n.strings.js (775 satır) varken.
6. **Status sözleşmesi dil-bağımsız değil** — UI Fransızca `Erreur`, Türkçe `Ağ bekleniyor`
   (App.vue:596-597 elle eşleme) ve İngilizce etiketleri birlikte eşliyor; history completion
   toast'u `Erreur`'yi ıska geçiriyor (171-172 vs failedNames:312 tutarsızlığı). Kök neden:
   backend metin-status üretiyor, kod/enum üretmeli.
7. **`apiPost` r.ok kontrolsüz** (api.js:27-33) — JSON dönen 4xx'ler başarı sanılabilir;
   cancel/pause/resume/remove hataları tamamen sessiz (App.vue:519-532).

### Küçük
8. **`seenHistory` Set'i sınırsız büyür** (uzun oturum belleği; toast tekrar bastırma riski) —
   App.vue:129,167.
9. **Pause/resume optimistik local status** — snapshot 30 sn'de düzeltir ama `queue` olayı status
   taşımaz → iki istemci arasında bayatlık penceresi (App.vue:531-532).
10. **Ayarlar sekmesi = Python WebUI paritesi kırık**
    - **Grid seçenekleri YANLIŞ**: SPA `2x4, 3x4, 4x3, 5x3` sunar (App.vue:944-945); izinli küme
      `{(3,3),(3,4),(4,3),(4,4)}` (rgsx_settings.py:445) → geçersiz değer yazılabiliyor; Rust
      `Settings::validate`'te allowed-set görünmüyor (settings.rs:151 yalnız default).
    - **qBittorrent WebUI bölümü YOK** (şifre yönetimi+durum): app.js:2359-2380 vs Rust uçları
      boşa düşmüş (lib.rs:74-77). **GÜNCELLEME (2026-08-24, kullanıcı kararı): bölüm
      EKLENMEYECEK** — torrent engine default'u librqbit (main.rs:47-50); `/api/qbittorrent/*`
      uçları yalnız legacy `RGSX_TORRENT_ENGINE=python` bridge modunda anlamlı, WebUI'ya ölü
      özellik paneli eklenmez. Uçların kendisinin emekliliği → TASK-012-gap-02 kapsamı.
    - **API key girişleri açık metin**: `type=text` (App.vue:1015-1020) vs Python maskeli
      `type=password` (app.js:2331+).
    - **Sistem bilgisi zayıf + yanlış kaynak**: Python ayrı `/api/system_info`'dan collapsible
      detay paneli çizer (app.js:2068-2192: cpu/sıcaklık/bellek/çözünürlük/partition/IP:port);
      SPA settings GET'inden düz satır listesi basar (App.vue:1023-1028); `/api/system_info`
      (lib.rs:50) hiç çağrılmıyor.
    - **Kaydetme modeli farklı**: Python tek 💾 Save butonu (app.js:2364-2377); SPA her
      `@change`'de sessiz otomatik kayıt — istem dışı ara kayıt riski.
    - **ROMS klasörü ipucu yok**: "Current: X (custom/default)" (app.js:2213-2215).
    - **SPA fazlalıkları** (Python webui'de yoktu; bilinçli ekleme mi drift mi — KARAR GEREKİyor):
      `light_mode`, `max_simultaneous_downloads`, `global_sort_option`, `symlink.target_directory`.
    - Ölü default alanlar: `display.monitor/fullscreen`, `accessibility.footer_font_scale` hiçbir
      UI'da yok (App.vue DEFAULT_SETTINGS:57,66).

## Fazlar

### Faz A — sessiz davranış hataları (en düşük risk)
- [x] `updateGamesList`: 🔄 butonu + sahte başarı toast'u KALDIRILDI (kullanıcı kararı
      2026-08-24: "butonu gizle"); endpoint gerçek işlev kazanana dek UI'da yok.
      Gerçek gamelist-refresh backend işi ayrı karar/görev (bulgu 2).
- [x] Grid seçenekleri `{3x3,3x4,4x3,4x4}` + Rust tarafında `normalize_grid` coercion
      (Python `set_display_grid` parity: küme dışı → "3x4"); `Settings::normalized()`
      public edildi, POST `/api/settings` kayıt yolu da normalize'dan geçer (bulgu 10-grid).
- [x] `apiPost` r.ok kontrolü + cancel/pause/resume/remove hata toast'ları
      (`action_failed` anahtarı 7 dile eklendi) (bulgu 7).
- [x] `mode` kararı: UI tek davranışa indirildi (kullanıcı kararı 2026-08-24) —
      ➕ butonu + `dlbtn.q` CSS + `dl_queue_title` i18n kaldırıldı; payload'dan `mode` düşürüldü.
      Backend'e 'now' desteği eklenMEDİ (bulgu 4).
- [x] `seenHistory` üst sınırı (500, FIFO düşüm) (bulgu 8).

### Faz B — TV modunu resmen emekliye ayır (SEÇENEK A, kullanıcı onaylı)
- [x] ~~`rgsx-webui-spa` SKILL.md düzenlemesi~~ **N/A**: skill dosyası diskte yok
      (yalnız `FAZ12_PARITY_STRATEGY.md` referansı var); `?mode=tv` SPA'da kod olarak
      zaten yok, retire kararı dokümana işlendi.
- [x] `/api/es-input` HTTP ucu: **söküm KULLANICI KARARIYLA iptal edildi** (2026-08-24) —
      uç tutuldu. Not: bugünkü TVUI in-process `es_input::load_best()` ile besleniyor
      (`main.rs:358` → `start_native_input`), uçtan geçmez; Python WebUI'de de tüketici
      yoktu. Karar gerekçesi: gelecek ayrık-süreç TVUI / uzak tüketici / ayarlar-UI
      gamepad bölümü senaryoları. Contract `test_es_input_shape` yerinde (114 baseline).
- [x] ~~`rgsx-faz12-migration` SKILL.md satır düzeltmesi~~ **N/A**: skill dosyası yok;
      `FAZ12_PARITY_STRATEGY.md` baseline satırı 114'e güncellendi + uç-kararı notu,
      `ROADMAP_FAZ12_RUST_WEBUI_TVUI.md` §0 superseded bloğu zaten günceldi.
- [x] `manager-bin/src/main.rs` bayat "SPA kiosk/webview" yorumları SDL2-shell
      gerçekine göre düzeltildi.

### Faz C — parite tamamlama
- [x] Self-update WebUI banner'ı (TVUI parity; bulgu 3) — snapshot `manager_update` nested +
      `manager_update` SSE olayı; available→İndir / downloading→%+İptal / ready→Yükle
      (tek tık, TVUI parity ikinci onay yok).
- [x] Ayarlar sekmesi: Save-buton modeli (kullanıcı kararı: Python parity; `@change`
      otomatik kayıt + onApiKey tuş-başı POST kaldırıldı), password maskeleme (api_keys
      type=password), collapsible system-info (`<details>`, /api/system_info), ROMS ipucu
      ("Mevcut: X (özel/varsayılan)"), fazlalık alanları TUTULUR (kullanıcı kararı;
      max_downloads backend'de işlevsel) (bulgu 10).
      qBittorrent bölümü EKLENMEZ (kullanıcı kararı — librqbit default, ölü özellik).
- [x] i18n temizliği: boot ekranı + Region-priority modalı literal'ları tt()'ye alındı;
      27 yeni anahtar × 7 dil. Status sözleşmesi: `manager-core::contract`'e
      `status_code`/`with_status_code`/`inject_status_codes(_into)` — snapshot, `queue`
      delta olayı, `/api/history`, `/api/queue` enjeksiyonlu; UI `itemStatus()` ile önce
      koda bakar, metin-map fallback kalır (bulgular 5, 6).
      **Bonus:** c495461'de action_failed anahtarları commit'e girmemişti (shipped bug:
      toast'lar ham "action_failed" basardı) — 7 dilde yeniden eklendi.
- [x] Pause/resume bayatlık penceresi: SSE `queue` olayı global `status` taşıyor VE
      status değişimi tek başına olay tetikliyor (`last_status` takibi); optimistik set
      kalır (kullanıcı kararı) (bulgu 9).
      **Bonus:** `test_settings_native_roundtrip` paralel flake'i kökünden kapatıldı —
      contract testlerine ENV_LOCK (settings/scan env yarışı; manager-core deseni).

## Doğrulama

- `cd webui && npm run build` (dist/ repoya commitli — build sonrası dist değişimi de commit edilir).
- Canlı: `manager-bin` → tarayıcı `http://<ip>:<port>/`; smoke listesi: 🔄 butonu davranışı,
  grid seçim kaydet/oku, ayarlar sekmesi Python ekran görüntüsüyle yan yana, `?mode=tv` +
  gamepad gezinme, self-update akışı (RGSX_UPDATE_MANIFEST_URL setken).
- Contract testleri etkilenmemeli (SPA saf frontend); `cargo test -p manager-http` yeşil kalır.

## İlerleme

- 2026-08-22 — İnceleme tamamlandı; bulgular 1-10 + faz planı bu dosyaya yazıldı. Uygulama
  kullanıcı onayıyla Faz A'dan başlayacak.
- 2026-08-22 — Bulgu 1 düzeltildi: "kaybolan regresyon" değil, native TVUI (yön B) kararından
  önceki dönemin emekliliği planlanmamış mirası. Kullanıcı onayıyla **Seçenek A** kilitlendi:
  `?mode=tv` resmen retire; Faz B buna göre yeniden yazıldı. (Ders: yön değişikliği kararı
  alındığı anda eski yönün aksiyonları da plana girilmeli.)
- 2026-08-24 — **Faz A uygulandı** (karar: 🔄 butonu gizle + mode tek-davranış; bkz. Faz A
  checklist). Rust: `manager-core/src/settings.rs` `normalize_grid` + `Settings::normalized()`,
  `manager-http/src/api.rs` settings_post normalize yoluna bağlandı. WebUI: App.vue (🔄 butonu
  kaldırımı, tek ⬇️, apiPost r.ok + hata toast'ları, seenHistory 500 sınırı, grid options,
  ölü `.dlbtn.q` CSS), api.js, i18n.strings.js (3 ölü anahtar 7 dilde silindi,
  `action_failed` 7 dile eklendi). dist/ yeniden build edildi.
  **Bonus düzeltme:** settings testlerindeki RGSX_SETTINGS_PATH env yarışı kapatıldı
  (ENV_LOCK; `background_theme_roundtrip_persisted` paralel koşumda düzenli kırmızıydı).
  **Doğrulama:** `cargo test -p manager-core --lib` 73/73, `cargo test -p manager-http`
  yeşil (28 lib + 114 contract + smoke), `webui npm run build` sıfır hata.
- 2026-08-24 — **Faz B uygulandı.** main.rs TVUI yorumları SDL2 gerçekine çevrildi;
  skill-dosyası maddeleri N/A (diskte yoklar). `/api/es-input` sökümü kullanıcı kararıyla
  iptal edildi — uç + `test_es_input_shape` yerinde (bkz. Faz B madde 2). Docs:
  PROJECT_MAP contract sayıları 114'e, FAZ12_PARITY_STRATEGY baseline + uç-kararı notu.
  **Doğrulama:** `cargo test -p manager-http` yeşil (114 contract dahil).
- 2026-08-24 — **Faz C uygulandı** (4 kullanıcı kararı: Save butonu / fazlalık tut /
  backend status_code / queue status; qBittorrent bölümü kullanıcı kararıyla düşürüldü —
  librqbit default). Rust: manager-core contract status_code yardımcıları + sse.rs
  snapshot/queue-olay enjeksiyonu + global status + api.rs history/queue. WebUI: upd
  banner + manager_update handler, itemStatus() kod-öncelik, settings Save-butonu,
  maskeli api_keys, roms ipucu, sysinfo `<details>`, boot/region i18n, 27 anahtar × 7 dil
  (+ action_failed c495461 eksiği telafi). **Bonus:** contract ENV_LOCK (settings
  roundtrip flake kökten kapatıldı). **Doğrulama:** manager-core 75/75, manager-http
  yeşil (28 lib + 114 contract + smoke'lar), `npm run build` sıfır hata, dist commit'li.
