# TASK-003 — Faz 11: İlk açılışta sistem dili otomatik algılama

- **id:** TASK-003
- **title:** Faz 11 — İlk Açılışta Sistem Dili Otomatik Algılama
- **status:** todo
- **priority:** P2
- **created:** 2026-08-11
- **tags:** language, i18n, rgsx-settings, tvui, rgsx-web

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 11 ("Planlandı"; tasarım
  `ROADMAP.md`'deki Faz 8 maddesinden taşındı)

## Açıklama

**Amaç:** İlk açılışta sistem dilini otomatik algılayıp o dile başlamak; kullanıcının dil
değiştirme ayarını ASLA bozmamak; tercümesi olmayan dilin İngilizce'ye düşmesini garanti etmek.

**Bugünkü durum (kodla doğrulandı):** algılama yalnızca Batocera'da çalışıyor
(`detect_batocera_language` language.py:358, `initialize_language` language.py:392).
Windows/Linux masaüstünde ilk açılış her zaman `en` oluyor. Desteklenmeyen dil kodu (ör.
sistem `ru_RU` → `ru`) settings'e **kalıcı** yazılıyor; her açılışta warning + `en` fallback
ama settings'te `ru` kalıyor (gerçek kullanılan dili yansıtmıyor). `detect_system_language`,
`language_mode`, `language_fallback_notified` kodda yok.

**KÖK SORUN (tasarım):** Otomatik algılanan dil ile kullanıcının bilinçli seçimi settings'te
AYNI alanda (`language`) temsil ediliyor. Desteklenmeyen dile auto-fallback kalıcı yazılıyor,
"ilk açılış" dosya varlığına bakıyor, `config.current_language` yalnızca menü değişiminde set
ediliyor.

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
- (A) Fallback'te eski auto `language` key'i silinir (WebUI/TVUI tutarlılığı).
- (B) Tek seferlik log/toast için kalıcı marker (`language_fallback_notified`).
- (C) Bildirim display-init sonrasına ertelenir.
- (D) **Migration karar noktası — KARAR VERİLDİ:** eski kurulumlarda `language` her zaman
  yazılmış olduğundan katı "hepsi manual" kuralı auto-detect'i hiçbir mevcut kullanıcıda
  çalıştırmaz. Kural: eski `language=="en"` → `mode="auto"` olarak migrate et;
  `language!="en"` → manual (kullanıcı tercihi korunur). Migrate edilen ilk boot'ta
  auto-detect çalışır ve:
  - algılanan dil de `"en"` ise → hiçbir şey yazma, bildirim GÖSTERME (sonuç aynı),
  - algılanan dil `"en"`den FARKLIYSa ve çeviri destekleniyorsa → settings güncelle +
    `language_fallback_notified` marker'ı ile tek seferlik bildirim göster.
- (E) Ortam sınıflandırıcı sinyalleri tanımlanır (`TERMUX_VERSION`/`PREFIX`).
- `language_mode=="manual"` ama key eksik (bozuk durum) → onarım log'lanır, auto-detect'e düşer.

## Kapsam / Dosyalar

- `language.py` — `detect_system_language`, `initialize_language` yeniden yazım
- `rgsx_settings.py` — `get_language` + `language_mode` persister
- `rgsx_web/i18n.py` — settings'ten okumayı doğrula
- `tvui.py` — bildirim bayrağı + ana döngü toast
- `tests/test_language.py` — (yeni)

## Doğrulama

`tests/test_language.py`: yeni kurulum + desteklenen OS dili (tr) → auto tr; yeni kurulum +
desteklenmeyen OS dili (ru) → in-memory en, key YOK, mode auto; manuel seçim → manual yazılır
sonraki boot'larda korunur; eski settings regression: `language=="tr"` (mode yok) → manual kalır,
`language=="en"` (mode yok) → mode auto'ya migrate edilir ve algılanan dil de en ise hiçbir şey
yazılmaz/bildirim gösterilmez, farklı dil ise settings güncellenir + tek seferlik bildirim;
Batocera dışı + Termux/RetroBat env mirası için ayrı test; mevcut suite baseline ile aynı kalır
(246 passed / 23 pre-existing — güncel sayı oturum özetinden alınır).

---

## İlerleme

- 2026-08-11 — Roadmap'ten tasks/ yapısına taşındı (todo; tasarım tamam, uygulama yok —
  `detect_batocera_language` + `initialize_language` dışında kod karşılığı bulunmuyor).
