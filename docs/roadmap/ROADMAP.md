# RGSX Geliştirme Yol Haritası (ROADMAP)

Tespit edilen iyileştirmeler, fayda/maliyet dengesine göre fazlara ayrılmıştır.
Her faz bağımsız commit olarak uygulanır, doğrulanır ve push edilir.

---

## Faz 1 — Systray "Sunucu Ayarları" Penceresi (Tkinter) ✅ TAMAMLANDI

**Amaç:** Systray "Ayarlar" menüsünde WebUI ayarlar sayfasını açmak yerine, sunucu
seviyesindeki ayarlar için küçük bir açılır pencere (Tkinter dialog) sunmak.

**Neden?**
- WebUI ayarlarında port/host değiştirmek servis restart'ı gerektirir; değişiklik sırasında
  web arayüzü bağlantısı kopar (chicken-egg). Systray'dan bağımsız bir pencere bu sorunu çözer.
- tkinter stdlib'de mevcut (`python -c "import tkinter"` → 8.6), ek bağımlılık yok, `pythonw` ile çalışır.

**Tasarım kararı (karma yaklaşım):**
- "Ayarlar" menüsü → WebUI `/settings` (mevcut davranış, oyun/uygulama ayarları) kalır.
- Yeni menü öğesi **"Sunucu Ayarları..."** → Tkinter dialog açılır:
  - `port` (WebUI + manager portu)
  - `host` (0.0.0.0 / 127.0.0.1)
  - `auto-start on boot` (toggle)
  - "Kaydet ve Yeniden Başlat" butonu → ayarları yazar, servisi restart eder.
  - Mevcut durum bilgisi: çalışıyor/duraklatma sayısı.

**Dosyalar:** `rgsx_manager.py` (yeni `settings_dialog.py` yardımcı modülü), `rgsx_settings.py` (host/port persister).

**Doğrulama:** port 5000→5001→5000 canlı geçişi ve restart spawn test edildi; manager sağlıklı.

---

## Faz 2 — HTTP İndirme Resumé (Range Desteği) ✅ TAMAMLANDI

**Amaç:** `download_rom` için `Range: bytes=` destekli kısmi indirme ve `Content-Range` takibi.

**Neden?**
- Torrentler qBittorrent'te kaldığı yerden devam ediyor; HTTP (1Fichier/Switch vs.) kesintide baştan başlıyor.
- 1Fichier'deki büyük dosyalar (Switch 875, Wii U, Windows 1805) tek seferde inemeyince tamamen kayboluyor.

**Kapsam:** `network.py` HTTP indirme akışına parça takibi (`.part` dosyası + resume offset),
sunucu Range desteklemiyorsa (206 yok) eski davranışa düşüş. İlerleme çubuğu aynı kalır.

**Uygulama:** `_stream_response_to_path` + `download_from_1fichier` akışı `.part` dosyasına yazar,
tamamlanınca hedefe rename eder; başlangıçta mevcut `.part` boyutu `Range: bytes=N-` header'ı ile
gönderilir, sunucu 206 dönerse kaldığı yerden devam eder (200 dönerse dosya baştan iner).
`Content-Range` toplam boyutu (toplam size) için parse edilir; ilerleme çubuğu resume'dan itibaren
doğru başlar. Yerel HTTP simülasyonu ile doğrulandı (tam indirme, 206 resume, 200 fallback).

**Dosyalar:** `network.py` (`_http_part_path`, `_http_resume_offset`, `_http_parse_content_range`).

---

## Faz 3 — qBittorrent WebUI Şifre Yönetimi ✅

**Amaç:** Varsayılan `admin`/`RGSXqbt` şifresinin ilk çalıştırmada değiştirilmesi/uyarılması.

**Neden?** Sabit varsayılan şifre port 18572'yi herkese açık bir torrent yönetim kapısı yapar.

**Kapsam:** Varsayılan şifre otomatik değiştirilmez; kullanıcı varsayılan şifreyle kullanırken
WebUI'da uyarı banner'ı + şifre değiştirme modal'ı, TVUI Ayarlar menüsünde şifre yönetimi gösterilir.
Değiştirilen şifre `rgsx_settings.json`'a kaydedilir, backend her login'de settings'ten okur.

**Uygulama:** `rgsx_settings.py` (`get/set_qbittorrent_webui_password`), `qbittorrent_backend.py`
(`get_password_status`, `change_webui_password`, `_get_configured_password`), `rgsx_manager.py`
(GET `/api/qbittorrent/password-status` + POST `/api/qbittorrent/change-password`), `rgsx_web.py`
(banner + modal, `data-translate`), `app.js` (kontrol + kaydet, `t()`), TVUI `controls.py`/`display.py`/
`__main__.py` (`pause_qbt_password` state + sanal klavye), çeviri anahtarları 7 dilde.

---

## Faz 4 — Port Çakışma Yönetimi ✅

**Amaç:** Sabit port 5000 doluysa otomatik alternatif port deneme / net hata.

**Kapsam:** `rgsx_manager.py` başlangıcında port serbest değilse 5000+N dene; gerçek portu
`rgsx_settings.json` ve manager loguna yaz; `__main__.py` `manager_port`'u buradan okusun.

**Uygulama:** `_find_available_port` (port doluysa preferred+N aralığında serbest port bulur,
hiçbiri yoksa net hata 0 döner); `main()`'de `manager_healthy` kontrolü sonrası alternatif porta
geçer ve `set_manager_port` ile kalıcılaştırır; `__main__.py` `ensure_manager` poll döngüsü her
turda settings'ten gerçek portu yeniden okur; `run_server`'a `kill_conflicts=False` eklendi —
manager başka bir uygulamanın process'ini asla öldürmez (eski kill davranışı shim/standalone
`rgsx_web.py`'de korundu). Canlı test: 5000 işgal edilince manager 5001'e geçti, işgalci
hayatta kaldı, settings 5001'e yazıldı; temizlik sonrası 5000'e döndü.

---

## Faz 5 — History I/O İyileştirmesi ✅ TAMAMLANDI

**Amaç:** `_set_bulk_history_status` senkron tüm-history disk yazımını azaltmak.

**Neden?** Pause/Resume sırasında her çağrı tüm history'yi yazıyor; history büyüdükçe gecikme artar.

**Kapsam:** Async yazma (thread/queue) veya sadece değişen kayıtları güncelleme + throttle.

**Uygulama:** `history.py` async batched writer (throttle 500ms, batch writes, async thread, shutdown'da flush).

---

## Faz 6 — Thread Güvenliği (download_threads / pause_events) ✅ TAMAMLANDI

**Amaç:** Paylaşılan sözlükleri tek `threading.Lock` ile korumak.

**Neden?** GIL crash'i önler ama yarış durumları (pause sırasında yeni thread başlaması) tutarsız durum yaratabilir.

**Uygulama:** `thread_safety.py` modülü (RLock tabanlı context manager'lar + `with_network_lock` dekoratörü), `network.py` bunu kullanır. Yinelenen kilit tanımları temizlendi (commit 33c551c) — %100 kapsam.

---

## Faz 7 — Hijyen ✅ TAMAMLANDI

- `rgsx_cli.py:842` eski `return`-in-`finally` SyntaxWarning'ı temizle.
- Systray menü string'lerini ("Ayarlar", "İndirmeleri Durdur/Sürdür") `language.py` çeviri sistemine bağla.

---

## Faz 8 — İlk Açılışta Sistem Dili Otomatik Algılama (TAŞINDI)

> Bu madde **ROADMAP_DOWNLOAD_MANAGER.md → Faz 11** olarak taşındı (o belge aktif
> roadmap'tir; tasarım aynen orada korunuyor). Bu dosya tamamlanmış roadmap'tir.

**Amaç:** İlk açılışta sistem dilini otomatik algılayıp o dile başlamak; kullanıcının
dil değiştirme ayarını ASLA bozmamak; tercümesi olmayan dilin İngilizce'ye düşmesini
garanti etmek.

**Neden?**
- Bugün algılama yalnızca Batocera'da çalışıyor (`detect_batocera_language`);
  Windows/Linux masaüstünde ilk açılış her zaman `en` — sistem dili Türkçe/Fransızca
  vb. olsa bile.
- Mevcut akışta tercümesi olmayan bir dil kodu (ör. sistem `ru_RU` → `ru`, dosyası yok)
  settings'e **kalıcı** yazılıyor; her açılışta warning + `en` fallback ama settings'te
  `ru` kalmaya devam ediyor (gerçek kullanılan dili yansıtmıyor).

**KÖK SORUN (tasarım):** Otomatik algılanan dil ile kullanıcının bilinçli seçimi
settings'te AYNI alanda (`language`) temsil ediliyor. Desteklenmeyen dile auto-fallback
kalıcı yazılıyor, "ilk açılış" dosya varlığına bakıyor, `config.current_language` yalnızca
menü değişiminde set ediliyor.

**İSTENEN DAVRANIŞ (tasarım kararı):**
- Settings şeması iki ayrı alan: `language` (kullanıcının explicit seçimi, yoksa key yok)
  + `language_mode` (`"auto"` | `"manual"`).
- **Geriye dönük uyumluluk:** eski dosyada `language` var ve `language_mode` yok → "manual"
  say. Var olan kullanıcı tercihi auto-detect ile asla ezilmez.
- **Boot sırası (display init'ten ÖNCE, tvui.py:140 → init_display :180):**
  1. `language` key'i VAR VE `language_mode=="manual"` → o dil kullanılır, algılama YAPILMAZ.
  2. HAYIR (key yok VEYA `mode=="auto"`) → algıla:
     - Batocera: `batocera-settings-get system.language` (env'e güvenilmez)
     - Genel OS: `locale.getlocale()` → env `LANG`/`LC_ALL`/`LC_MESSAGES` → `getdefaultlocale()`
       (deprecated, en son çare)
     - Termux/RetroBat: host'tan miras env'i "gerçek sistem dili" SANMA; ayrı logla,
       en düşük öncelik (ortam sınıflandırıcısı: `TERMUX_VERSION`/`PREFIX` gibi sinyaller).
  3. Kodu normalize et: `tr_TR.UTF-8` → önce `tr_TR` sonra `tr` zinciri.
  4. Çeviri VARSA → `language=<kod>, language_mode="auto"` yaz (hâlâ auto — kullanıcı seçmedi).
     Çeviri YOKSA → `language` key'ini **SİL** (varsa), `language_mode="auto"` kalsın;
     `config.current_language="en"` yalnızca bellekte (kalıcı değil). **Silme şarttır:**
     WebUI `rgsx_settings.get_language()`'den direkt okur (rgsx_settings.py:588, i18n.py),
     eski auto key kalırsa TVUI=tr WebUI=ru/en tutarsızlığı oluşur.
- **Precedence:** explicit manual > OS/Batocera locale > shell-miras env > `en` (yalnızca in-memory).
- **Menü değişimi:** `language=<seçim>, language_mode="manual"` yaz → bir daha hiç algılanmaz.
- **`config.current_language`:** boot'un sonunda (adım 2 bitince) set edilir; menüdeki set
  yalnızca runtime değişimi içindir.
- **Tek seferlik uyarı:** auto→en-fallback geçişi kalıcı `language_fallback_notified` marker'ı
  ile bir kez loglanır + gösterilir (marker yoksa her boot'ta tekrar eder). Bildirim
  init_display'den önce üretilir → `config.language_fallback_notify` bayrağına yazılıp ana
  döngüde display hazır olunca toast/banner olarak gösterilir.

**Kör nokta düzeltmeleri (tasarımda tespit edilen):**
- Fallback'te eski auto `language` key'i silinir (A — WebUI/TVUI tutarlılığı).
- Tek seferlik log/toast için kalıcı marker (B).
- Bildirim display-init sonrasına ertelenir (C).
- **Migration karar noktası (D) — KARAR VERİLDİ:** eski kurulumlarda `language` her zaman
  yazılmış olduğundan katı "hepsi manual" kuralı auto-detect'i hiçbir mevcut kullanıcıda
  çalıştırmaz. Kural: eski `language=="en"` → `mode="auto"` olarak migrate et;
  `language!="en"` → manual (kullanıcı tercihi korunur). Migrate edilen ilk boot'ta
  auto-detect çalışır ve:
  - algılanan dil de `"en"` ise → hiçbir şey yazma, bildirim GÖSTERME (sonuç aynı,
    kullanıcıya gösterilecek "değişiklik" yok),
  - algılanan dil `"en"`den FARKLIYSa ve çeviri destekleniyorsa → settings güncelle +
    `language_fallback_notified` marker'ı ile tek seferlik bildirim göster.
- Ortam sınıflandırıcı sinyalleri tanımlanır (E).
- `language_mode=="manual"` ama key eksik (bozuk durum) → onarım log'lanır, auto-detect'e düşer.

**Dosyalar:** `language.py` (`detect_system_language`, `initialize_language` yeniden yazım),
`rgsx_settings.py` (`get_language` + `language_mode`), `rgsx_web/i18n.py` (settings'ten
okumayı doğrula), `tvui.py` (bildirim bayrağı + ana döngü toast).

**Doğrulama:** `tests/test_language.py` — yeni kurulum + desteklenen OS dili (tr) → auto tr;
yeni kurulum + desteklenmeyen OS dili (ru) → in-memory en, key YOK, mode auto; manuel seçim →
manual yazılır sonraki boot'larda korunur; eski settings regression: `language=="tr"` (mode yok)
→ manual kalır, `language=="en"` (mode yok) → mode auto'ya migrate edilir ve algılanan dil de
en ise hiçbir şey yazılmaz/bildirim gösterilmez, farklı dil ise settings güncellenir + tek
seferlik bildirim; Batocera dışı + Termux/RetroBat env mirası için ayrı test; mevcut suite
baseline ile aynı kalır (183 passed / 23 pre-existing).

---

## Önerilen Sıra

Faz 1 → Faz 2 → Faz 3 → Faz 4 → Faz 5 → Faz 6 → Faz 7 → **Faz 8 (taşındı → ROADMAP_DOWNLOAD_MANAGER.md Faz 11)**
