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
1. **TV modu + gamepad navigasyonu KAYBOLMUŞ (regresyon)** — ilk App.vue commit'i `b5827d2`'de
   `getGamepads`/`?mode=tv` kodu mevcut (git show: 2 isabet); bugünkü dosyada SIFIR.
   `/api/es-input` ucu (`lib.rs:54`) tam bunun için yazılmıştı (TASK-005-B notu: "webui TV modu
   tüketir") ve artık TÜKETİCİSİZ. `manager-tvui` wry'siz kurulumda harici tarayıcıyı `?mode=tv`
   ile açar → TV kullanıcısı desktop layout + gamepadsiz kalır. Skill dokümanı
   (rgsx-webui-spa) da gerçeği yansıtmıyor → güncellenmeli.
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
      boşa düşmüş (lib.rs:74-77).
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
- [ ] `updateGamesList`: GET'e çevir + sahte başarı toast'u kaldır; endpoint gerçek işlev
      kazanıncaya kadar buton dürüst davransın (hata mesajı veya gizleme). Gerçek gamelist-refresh
      backend işi ayrı karar/görev (bulgu 2).
- [ ] Grid seçenekleri `{3x3,3x4,4x3,4x4}` + Rust `Settings::validate` allowed-set doğrulaması
      (bulgu 10-grid).
- [ ] `apiPost` r.ok + cancel/pause/resume/remove hata bildirimi (toast) (bulgu 7).
- [ ] `mode` kararı: backend'e gerçek 'now' desteği EKLE ya da UI'ı tek davranışa indir (dürüstlük);
      karar bu görevde netleşecek (bulgu 4).
- [ ] `seenHistory` üst sınırı + tekrar-toast politikası (bulgu 8).

### Faz B — TV modu kurtarma
- [ ] `?mode=tv` + gamepad navigasyonunu `b5827d2` sürümünden kurtar, `/api/es-input` ile ES-map
      senkronu bağla (bulgu 1); rgsx-webui-spa skill'ini güncelle.

### Faz C — parite tamamlama
- [ ] Self-update WebUI banner'ı (TVUI parity; bulgu 3).
- [ ] Ayarlar sekmesi paritesi: qBittorrent bölümü, password maskeleme, collapsible system-info
      (/api/system_info), Save-buton modeli kararı, ROMS ipucu, fazlalık alanlarının kaderi
      (bulgu 10).
- [ ] i18n temizliği (tt() dışındaki tüm literal'lar) + status sözleşmesini kod-bazına alma kararı
      (bulgular 5, 6).
- [ ] Pause/resume bayatlık penceresi: `queue` olayına status ekle ya da optimistik set'i kaldır
      (bulgu 9).

## Doğrulama

- `cd webui && npm run build` (dist/ repoya commitli — build sonrası dist değişimi de commit edilir).
- Canlı: `manager-bin` → tarayıcı `http://<ip>:<port>/`; smoke listesi: 🔄 butonu davranışı,
  grid seçim kaydet/oku, ayarlar sekmesi Python ekran görüntüsüyle yan yana, `?mode=tv` +
  gamepad gezinme, self-update akışı (RGSX_UPDATE_MANIFEST_URL setken).
- Contract testleri etkilenmemeli (SPA saf frontend); `cargo test -p manager-http` yeşil kalır.

## İlerleme

- 2026-08-22 — İnceleme tamamlandı; bulgular 1-10 + faz planı bu dosyaya yazıldı. Uygulama
  kullanıcı onayıyla Faz A'dan başlayacak.
