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

## Faz 5 — History I/O İyileştirmesi

**Amaç:** `_set_bulk_history_status` senkron tüm-history disk yazımını azaltmak.

**Neden?** Pause/Resume sırasında her çağrı tüm history'yi yazıyor; history büyüdükçe gecikme artar.

**Kapsam:** Async yazma (thread/queue) veya sadece değişen kayıtları güncelleme + throttle.

---

## Faz 6 — Thread Güvenliği (download_threads / pause_events)

**Amaç:** Paylaşılan sözlükleri tek `threading.Lock` ile korumak.

**Neden?** GIL crash'i önler ama yarış durumları (pause sırasında yeni thread başlaması) tutarsız durum yaratabilir.

---

## Faz 7 — Hijyen

- `rgsx_cli.py:842` eski `return`-in-`finally` SyntaxWarning'ını temizle.
- Systray menü string'lerini ("Ayarlar", "İndirmeleri Durdur/Sürdür") `language.py` çeviri sistemine bağla.

---

## Önerilen Sıra

Faz 1 → Faz 2 → Faz 3 → Faz 4 → Faz 5 → Faz 6 → Faz 7
