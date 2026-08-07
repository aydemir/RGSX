# RGSX Özellikler ve Değişiklik Günlüğü

## Port Çakışma Yönetimi (2026.08.07)

**Olay:** RGSX Download Manager artık istenen port (varsayılan 5000) başka bir uygulama
tarafından kullanılıyorsa onu öldürmek yerine otomatik olarak `5000+N` aralığında serbest bir
porta geçiyor (Faz 4).

**Neden:** Eski davranış port doluysa o process'i `taskkill`/`kill -9` ile öldürüyordu — başka bir
uygulamanın (tarayıcı, sunucu vb.) çökmesine yol açabiliyordu. Faz 4 bunu zararsız bir alternatif
port seçimine çevirir.

**Yapılan değişiklikler:**
- **`rgsx_manager.py`:** `_is_port_free()` (bind testi, SO_REUSEADDR'sız — Windows'ta güvenilir) +
  `_find_available_port(preferred)` (preferred doluysa preferred+1..+100 arar, hiçbiri yoksa 0 döner).
  `main()`'de `manager_healthy` kontrolü sonrası alternatif porta geçer, `set_manager_port()` ile
  kalıcılaştırır, log'a yazar; `run_server(...)` artık `kill_conflicts=False` ile çağrılır.
- **`rgsx_web.py`:** `run_server(..., kill_conflicts=True)` parametresi — manager `False` geçer
  (başka process öldürmez); standalone/shim web sunucusu eski kill davranışını korur.
- **`__main__.py`:** `ensure_manager()` poll döngüsü her turda `get_manager_port()`'u yeniden okur —
  manager alternatif porta geçince TVUI/SSE doğru porta bağlanır.

**Doğrulama:** 5000'i işgal eden ayrı bir process ile canlı test — manager otomatik 5001'e geçti,
`manager_port=5001` settings'e yazıldı, health 5001'de OK, işgalci süreç öldürülmedi. Temizlik
sonrası manager 5000'e döndü.

---

## qBittorrent WebUI Şifre Yönetimi (2026.08.07)

**Olay:** Varsayılan `admin`/`RGSXqbt` şifresi otomatik değiştirilmiyor; kullanıcı varsayılan şifreyle
kullanırken uyarılıyor ve WebUI + TVUI üzerinden şifresini güncelleyebiliyor (Faz 3).

**Neden:** Sabit varsayılan şifre, qBittorrent WebUI (18572) için herkese açık bir yönetim kapısıdır.
Şifreyi kullanıcı seçmeli, sistem otomatik üretip gizlememeli.

**Yapılan değişiklikler:**
- **`rgsx_settings.py`:** `get_qbittorrent_webui_password()` (settings anahtarı yoksa config sabitine
  düşer → "varsayılan kullanımda" tespiti) + `set_qbittorrent_webui_password()`.
- **`qbittorrent_backend.py`:** `_CONFIGURED_PASSWORD` sabiti kaldırıldı; her login'de settings'ten
  dinamik okuma (`_get_configured_password`). `get_password_status()` (banner için using_default) +
  `change_webui_password()` (min 6, qB çalışıyorsa setPreferences, kalıcı settings kaydı).
- **`rgsx_manager.py`:** GET `/api/qbittorrent/password-status` + POST `/api/qbittorrent/change-password`.
- **`rgsx_web.py`:** `#qb-password-banner` (sarı uyarı + "Şifreyi Güncelle") + `#qb-password-modal`
  (yeni şifre + tekrar + hata alanı); metinler `data-translate` ile çevrilebilir.
- **`static/js/app.js`:** `checkQbittorrentPasswordStatus()` (DOMContentLoaded'de), modal aç/kapat,
  `saveQbittorrentPassword()` (min 6, eşleşme, POST, başarıda banner gizle + toast); mesajlar `t()`.
- **TVUI (`controls.py` / `display.py` / `__main__.py`):** Ayarlar menüsüne "qBittorrent WebUI Şifresi"
  satırı (`< Varsayılan >` / `< Özel >` durumu); `pause_qbt_password` ekranı — durum + maskeli giriş +
  4 satır AZERTY sanal klavye; kayıt manager'a POST (thread), sonuç popup.
- **`languages/*.json` (7 dil):** `qbt_password_*` (TVUI) + `web_qbt_password_*` (WebUI) anahtarları.

**Doğrulama:** `py_compile` + import + canlı manager üzerinde uçtan uca test: `using_default` True
varsayılan; `TestPass123` ile değiştirince False + settings'e yazıldı; kısa şifre 400; temizlik
`RGSXqbt`'ye geri döndürüldü + anahtar silindi → `using_default` tekrar True.

---

## Systray "Sunucu Ayarları" Penceresi (2026.08.06)

**Olay:** RGSX Download Manager systray menüsüne "Sunucu Ayarları..." Tkinter penceresi eklendi. WebUI `/settings` (oyun/uygulama ayarları) korundu; sunucu seviyesindeki ayarlar (port, host, auto-start) ayrı bir açılır pencereden yönetiliyor.

**Neden:** Port/host değiştirmek servis restart'ı gerektirir; o sırada WebUI bağlantısı kopar (chicken-egg). Systray'dan bağımsız Tkinter penceresi bu sorunu çözer. tkinter stdlib'de mevcuttur (`pythonw` ile ek bağımlılık yok).

**Yapılan değişiklikler:**
- **`settings_dialog.py` (yeni):** Tkinter modal dialog — port (doğrulama + 1-65535), host, auto-start toggle, "Kaydet ve Yeniden Başlat" / "İptal". Port doluysa uyarır; sistem tray'den ayrı thread'de açılır.
- **`rgsx_manager.py`:** Systray menüye "Sunucu Ayarları..." öğesi eklendi; `_on_server_cfg_saved` ayarları `rgsx_settings.json`'a yazar ve port/host değiştiyse `_restart_manager_for_settings()` ile servisi yeniden spawn edip kapatır. `main()` artık port/host'u CLI argümanı verilmediğinde kalıcı ayarlardan okur.
- **`rgsx_settings.py`:** `get/set_manager_port`, `get/set_manager_host` eklendi (varsayılan 5000 / 0.0.0.0).
- **`__main__.py`:** `ensure_manager()` manager portunu kalıcı ayarlardan okur (TVUI → manager delegasyonu doğru porta gider).

**Doğrulama:** `py_compile` + import + dialog smoke test geçti; port 5000→5001→5000 geçişi canlı manager'da uçtan uca doğrulandı (health endpoint her portta OK, restart spawn çalışıyor). Çalışan manager: v2.6.5.6, port 5000.

---

## Tray Menü + İndirme Duraklatma + Yeniden Başlatmada Sürdürme (2026.08.06)

**Olay:** RGSX Download Manager tray menüsüne yeni eylemler eklendi; indirmelerin bilgisayar/manager yeniden başlatıldığında kaldığı yerden devam etmesi sağlandı.

**Yapılan değişiklikler:**
- **Tray menüsü** (`rgsx_manager.py`): "Ayarlar" (WebUI `/settings` sayfasını açar) ve "İndirmeleri Durdur/Sürdür" menü öğeleri eklendi. Duraklatma durumunda menüde tik işareti (`checked`) gösterilir.
- **Toplu duraklatma/sürdürme** (`network.py`): `pause_all_downloads()`, `resume_all_downloads()`, `is_any_download_paused()` eklendi. Hem HTTP direkt hem torrent indirmelerini kapsar; aktif görevler için `pause_events` threading.Event set/clear eder ve history statülerini `Paused`/`Downloading` olarak günceller.
- **HTTP API** (`rgsx_manager.py`): `POST /api/pause` ve `POST /api/resume` endpoint'leri eklendi (WebUI/CLI üzerinden de tetiklenebilir).
- **Yeniden başlatmada sürdürme** (`rgsx_manager.py`): `_resume_interrupted_downloads()` — manager başlarken history'de "Téléchargement"/"Downloading"/"Paused" statüsündeki girdileri `config.download_queue`'ya geri ekler; torrentler qBittorrent'teki kısmi veriden kaldığı yerden devam eder.
- **Çift resume önleme** (`__main__.py`): TVUI yerel resume döngüsü `_resume_tvui_downloads()`'a taşındı; manager aktifken TVUI tarafı atlanır, resume sadece manager tarafından yapılır.

**Doğrulama:** `py_compile` + import + birim testleri (pause/resume/queue re-enqueue) geçti; deploy hedefinde manager v2.6.5.6 tray ile sağlıklı, `/api/health` OK, `/api/pause` + `/api/resume` 200 döndü.

---

## v2.6.5.6 Upstream Birleştirmesi (2026.08.06)

**Olay:** Upstream `RetroGameSets/RGSX` → `v2.6.5.6` (`0a317db`), custom `v2.6.5.2` tabanına merge edildi. Tüm custom özellikler (RGSX Download Manager, tray auto-start, SSE, WebUI oyun-durumu göstergeleri, ROM tarama, Türkçe) korunarak upstream'in yeni qBittorrent torrent altyapısı aktif edildi.

**Upstream'den gelenler:**
- Torrent motoru değişimi: aria2c → **gömülü qBittorrent** (`network.py` yeniden yazıldı, `qbittorrent_backend.py` eklendi; asset'ler: `qbittorrent-portable.7z`, `qbittorrent-nox_linux`).
- `version.json` ve `config.py`: `2.6.5.6`.
- Windows güncelleme mekanizması: `RGSX_update_windows_latest.zip` in-app uygulaması (`_apply_pending_windows_update`).
- qBittorrent WebUI butonu (`:18572`, `admin`/`RGSXqbt`) — web UI'da ve firewall scriptinde.
- TV UI: `Paused` durumu, seed durdurma ayrımı, ARM platform/1fichier uyarıları (`_open_selected_platform` refactor).

**Custom tarafta korunanlar / birleştirme kararları:**
- `__main__.py`: upstream `start_web_server()` çağrısı yerine custom mimari korundu — `ensure_manager()` + `_start_manager_sse_listener()` (web sunucusunu manager başlatır). Upstream'in `qbittorrent_backend` import'u ve torrent-resume prewarm (`_prewarm_qbittorrent_startup`) aynı akışa entegre.
- `controls.py`: platform açma upstream'in `_open_selected_platform()` (ARM/1fichier uyarıları dahil) ile yapılıyor; custom `scan_platform_roms_on_enter()` çağrısı bu fonksiyonun içine taşındı.
- `config.py`: `manager_port`/`manager_available` (custom) + `OTA_UPDATE_WINDOWS_ZIP`/`TORRENT_QBITTORRENT_WEBUI_PASSWORD` (upstream) birlikte.
- `display.py` / `rgsx_web.py` / `app.js`: her iki tarafın değişiklikleri de korundu (manager SSE yansıması + qBittorrent butonu; game-status göstergeleri + tab render iyileştirmeleri).

**Doğrulama:** Tüm Python dosyaları `py_compile` geçti; `network` API'si (download_rom, download_from_1fichier, download_queue_worker, cancel_all_downloads, request_cancel, ...) custom çağrıcılarıyla uyumlu doğrulandı.

---

## Yapılan Özelleştirmeler (RetroBat Entegrasyonu)

### Tray Auto-Start Varsayılan AÇIK (v2.6.5.2 sonrası)

**Dosyalar:** `rgsx_manager.py`, `rgsx_settings.py`

- `rgsx_settings.json`'a `autostart_on_boot` anahtarı eklendi (varsayılan `true`).
- Manager ilk başladığında (tray olmadan) tercih `true` ise Registry'ye otomatik kurulur.
- Kullanıcı tray'den kapatırsa tercih kalıcı olarak `false` yazılır; yeniden başlatmada açılmaz.
- `--auto-start-install` / `--auto-start-remove` da tercihi günceller.

---

### WebUI Platform Listesi Render/Yenilenme İyileştirmeleri

**Dosyalar:** `rgsx_web.py`, `static/js/app.js`

- SSE snapshot (~15 sn) sırasında platform grid'i yeniden render edilmiyor (imza karşılaştırması).
- Platform görüntüleri için oturum bazlı stabil `?v=` cache-buster (her render'da değişen `Date.now()` yerine oturum sabiti).
- `rgsx_web.py`: image yanıtlarında `Cache-Control: public, max-age=3600` (`no-store` kaldırıldı) → platform görselleri re-render'da tekrar indirilmiyor.
- 30 sn'lik auto-refresh artık `location.reload()` yerine sadece verileri HTTP ile yeniliyor (tam sayfa yeniden yüklemesi yok).
- Doğrulama: snapshot sonrası re-render yok, 148 platform görseli tek seferde yükleniyor.

---

### 🆕 Yüklü ROM'ları Gizle Filtresi (2026.08.07)

**Olay:** TVUI ve WebUI oyun listesinde "Yüklü ROM'ları Gizle" checkbox filtresi eklendi. Aktifken HDD'de zaten indirilmiş/kurulu oyunlar listeden gizlenir.

**Neden:** Kullanıcı "Hangi oyunları henüz indirmemişim?" diye hızlıca filtreleyebilmeli. Yeşil işaretleme (indirilmiş oyun göstergesi) filtre kapalıyken korunur.

**Yapılan değişiklikler:**

**TVUI (`game_filters.py`, `display.py`, `controls.py`):**
- `GameFilters` sınıfına `hide_downloaded` bayrağı eklendi (`__init__`, `load_from_dict`, `to_dict`, `is_active`, `reset`)
- `apply_filters(games, platform_name=None)`: `hide_downloaded` aktifken `is_game_downloaded(platform_name, game.name)` ile filtreleme
- `draw_filter_advanced()`: "Yüklü ROM'ları Gizle" toggle eklendi (one_rom_per_game ve priority_config arasına)
- `controls.py`: `num_other_options = 4`, option_idx 2 → `hide_downloaded` toggle, priority_config → 3
- `draw_game_list`, `_apply_sorted_active_filters`, `filter_games_by_search_query`: `platform_name` geçiriliyor
- Global search: item bazlı platform filtering (`platform_label`)

**WebUI (`rgsx_web.py`, `static/js/app.js`, `languages/*.json`):**
- `web_filter_hide_downloaded` çeviri anahtarı 7 dile eklendi (TVUI `filter_hide_downloaded` ile aynı metin: "Yüklü ROM'ları Gizle")
- HTML rendering: game-item'a `data-downloaded="${g.downloaded}"` attribute eklendi
- `applyAllFilters()`: `item.dataset.downloaded === 'true'` ile filtreleme (SSE status yerine API `downloaded` boolean)
- `saveFiltersToBackend` + settings save: `hide_downloaded` okuma/yazma (`??` operatörü ile checkbox false handling)
- `rgsx_web.py` `/api/save_filters`: `hide_downloaded` okuma/yazma + `config.game_filter_obj` güncelleme

**Çeviriler (7 dil):**
| Dil | `filter_hide_downloaded` (TVUI) | `web_filter_hide_downloaded` (WebUI) |
|-----|----------------------------------|--------------------------------------|
| TR  | Yüklü ROM'ları Gizle            | Yüklü ROM'ları Gizle                |
| EN  | Hide Downloaded ROMs            | Hide Downloaded ROMs                |
| FR  | Masquer les ROMs téléchargées   | Masquer les ROMs téléchargées       |
| DE  | Heruntergeladene ROMs ausblenden| Heruntergeladene ROMs ausblenden    |
| ES  | Ocultar ROMs descargadas        | Ocultar ROMs descargadas            |
| IT  | Nascondi ROMs scaricate         | Nascondi ROMs scaricate             |
| PT  | Ocultar ROMs baixadas           | Ocultar ROMs baixadas               |

**Davranış:**
- Tik açık: `is_game_downloaded()` True olan oyunlar listeden gizlenir
- Tik kapalı: Mevcut davranış korunur, indirilen oyunlar yeşil işaretlenerek gösterilir (`[>]` TVUI / `[✓]` WebUI)
- Yeşil işaretleme kodu (`draw_game_list` / WebUI status badge) dokunulmaz, sadece filtre kapalıyken görünür
- Kapsam: Mevcut platform (`config.current_platform` / platform-specific API)

**Doğrulama:** Deploy kopya (`C:\RetroBat - Kopya\roms\ports\RGSX`) + manager restart → TVUI filtre menüsünde checkbox görünüyor, WebUI "Yüklü ROM'ları Gizle" yazıyor, tik açıldığında indirilmiş oyunlar gizleniyor, kapatıldığında yeşil `[>]`/`[✓]` ile geri geliyor.

---

### v2.6.4.9-TR2 - Web UI Masaüstü Kısayolu

**Dosyalar:** `windows/RGSX Retrobat.bat`, `windows/create_shortcut.vbs`

Masaüstüne "RGSX Web UI" kısayolu oluşturarak web arayüzünü kolayca başlatabilirsiniz.

**Yeni BAT Seçenekleri:**
- `--webui` → Sadece web sunucusunu başlatır (TV UI çalışmaz)
- `--create-shortcut` → Masaüstüne kısayol oluşturur

**Kullanım:**
```batch
"RGSX Retrobat.bat" --create-shortcut   # Kısayol oluştur
"RGSX Retrobat.bat" --webui             # Sadece web sunucusu
```

**Oluşturulan Kısayol:**
- Hedef: `RGSX Retrobat.bat --webui`
- İkon: `favicon_rgsx.ico`
- Konum: `%USERPROFILE%\Desktop\RGSX Web UI.lnk`

**Avantajları:**
- Tek tıkla web arayüzüne erişim
- TV UI olmadan sadece web arayüzü çalıştırma
- Diğer kullanıcılar için otomatik kurulum

---

### v2.6.4.9-TR1 - Web UI Oyun Durum Göstergeleri

**Dosyalar:** `rgsx_web.py`, `static/js/app.js`

Web arayüzünde oyun listelerinde indirme durumu göstergeleri:

| Durum | İkon | Renk | Açıklama |
|-------|------|------|----------|
| İndirilmiş | `[✓]` | Yeşil `#66ff66` | Oyun indirilmiş |
| İndiriliyor | `[~] %` | Sarı `#ffcc00` | İndirme devam ediyor |
| Başarısız | `[✗]` | Kırmızı `#ff5555` | İndirme başarısız |
| Normal | yok | Tema rengi | Henüz indirilmemiş |

**API:** `GET /api/game-status` → Tüm oyunların durumunu döndürür

---

### v2.6.4.9-TR1 - Oyun Listesi Durum Göstergeleri

**Dosya:** `display.py`

Oyun listesinde indirme durumu renkli göstergelerle gösterilir:

| Durum | Prefix | Renk | Açıklama |
|-------|--------|------|----------|
| İndirilmiş | `[>]` | Yeşil `(100, 255, 100)` | `is_game_downloaded()` ile doğrulanmış |
| İndiriliyor | `[~] %sayı` | Sarı `(255, 200, 0)` | `config.download_tasks` + `download_progress` |
| Başarısız | `[X]` | Kırmızı `(255, 80, 80)` | `config.history` son deneme `Erreur`/`Error` |
| Normal | yok | Tema rengi | Henüz indirilmemiş |

**Örnek:** `[~] 45% After Burner` → sarı renkte, %45 indirilmiş.

**Mantık:**
- `config.download_tasks` → Aktif indirme görevleri (task_id → (task, url, game_name, platform))
- `config.download_progress` → İndirme ilerlemesi (url → {status, progress_percent, ...})
- `config.history` → Geçmiş indirmeler (başarısız olanlar kontrol edilir)

---

### v2.6.4.9-TR1 - Türkçe Dil Desteği

**Dosyalar:** `language.py`, `languages/tr.json`, `static/js/app.js`

- `language.py`: `get_language_name()` fonksiyonuna `"tr": "Türkçe"` eklendi
- `languages/tr.json`: Tam Türkçe çeviri dosyası (337+ anahtar)
- `static/js/app.js`: Web arayüzünde Türkçe dil seçeneği eklendi

**Desteklenen diller:** FR, EN, ES, DE, IT, PT, JA, ZH, RU, **TR**

---

### v2.6.4.9-TR1 - Performans Optimizasyonu

**Dosya:** `display.py`

Gradient ve grain texture önbellek (cache) sistemi eklendi:

```python
_gradient_cache = {"surface": None, "top": None, "bottom": None, "size": None}
_grain_cache = {"surface": None, "size": None}
```

- `_build_grain_surface()`: Grain texture'sını sabit seed (42) ile bir kez oluşturur
- `draw_gradient()`: Aynı parametrelerle her frame'de yeniden çizim yerine cache'den okur
- Büyük ekranlarda belirgin performans artışı sağlar

---

### v2.6.4.9-TR1 - İndirme İlerleme Düzeltmesi

**Dosya:** `display.py`

**Sorun:** Oyun listesinde indirme yüzde gösterilmiyordu. `download_tasks`'daki `game_name` uzantılı (örn: `"Oyun.rvz"`), `item.display_name` uzantısız (örn: `"Oyun"`) olduğu için eşleşme başarısız oluyordu.

**Çözüm:**
- `os.path.splitext()` ile uzantı kaldırılarak karşılaştırma yapıldı
- Fallback arama: `download_progress` dict'indeki `game_name` ile de uzantı kaldırılarak eşleştirildi

**Etkilenen durumlar:**
- İndirme yüzdesi artık tüm listelerde görünüyor
- BIOS listeleri gibi farklı kaynaklarda da çalışıyor

---

## RGSX Download Manager (v2.6.5.2)

**Yeni dosya:** `rgsx_manager.py`

TV UI (Pygame) ve indirme motoru aynı process'te çalışıyordu; TV UI kapatıldığında tüm indirmeler ölüyordu. Artık bağımsız bir **RGSX Download Manager** daemon'ı indirmeleri arka planda (sistem tepsisi / tray) yönetiyor.

**Mimari:**
- `rgsx_manager.py` → Bağımsız daemon. HTTP + SSE sunar, kuyruk işçi thread'i (`download_queue_worker`) çalıştırır, tepsi ikonu gösterir, Windows otomatik başlatma (Registry `Run` anahtarı) kurar.
- `rgsx_web.py` → Web sunucusu. `__main__` kısmı artık **shim**: manager sağlıklıysa 0 ile çıkar, değilse manager'ı arka planda başlatıp bekler.
- `__main__.py` → TV UI. `ensure_manager()` ile manager'ı garanti eder, SSE client ile manager durumunu `config.*`'a yansıtır.
- `rgsx_cli.py` → İndirme komutları manager sağlıklıysa HTTP ile delege edilir, değilse yerel fallback.
- `controls.py` → TV UI'de indirme istekleri manager'a delege edilir (`config.manager_available`).
- `display.py` → Manager tarafından yansıtılan `config.download_progress` ile oyun listesinde indirme göstergeleri.
- `static/js/app.js` → SSE (`/api/events`) ile canlı güncelleme; 30 sn'lik `snapshot` oyun listesini platform listesine döndürmez.

**Manager API:**
| Endpoint | Metot | Açıklama |
|----------|-------|----------|
| `/api/health` | GET | Manager durumu (`success`, `manager`, `version`, `pid`) |
| `/api/events` | GET (SSE) | `snapshot` / `progress` / `history` / `queue` / `downloaded` olayları |
| `/api/download` | POST | İndirme ekle (`game_index`, `game_name` veya doğrudan `url` ile) |
| `/api/cancel` | POST | İndirmeyi iptal eder (kuyruktan `pop` yapmadan, işçi thread'i ile senkronize) |
| `/api/shutdown` | POST | Manager'ı kapatır |

**Başlatma seçenekleri:**
- `python rgsx_manager.py` → Tepsi ikonlu çalıştır
- `--no-tray`, `--port=N`, `--minimized`, `--auto-start-install`, `--auto-start-remove`
- TV UI fallback: `--ui-only` argümanı veya `RGSX_NO_MANAGER=1` env → manager olmadan yerel kuyruk

**SSE Durum Yansıması:** Manager'daki değişiklikler TV UI'a `config.history`, `config.download_queue`, `config.download_active`, `config.download_progress`, `config.downloaded_games` olarak yansıtılır; `config.needs_redraw` ile yeniden çizim tetiklenir.

**Düzeltilen hatalar:**
- `__main__.py`: eksik `import json` → `_manager_healthy()` hep `False` dönüyordu (SSE yansıması da bozuktu).
- `rgsx_web.py`: `do_GET`/`do_POST` içindeki yerel `from history import load_history/save_history` import'ları modül seviyesini gölgeliyordu → `/api/history`, `/api/cancel`, `/api/queue/clear`, `/api/queue/remove` `UnboundLocalError` veriyordu.
- Web UI: SSE 30 sn'lik `snapshot` olayı oyun listesini silip platform listesine döndürüyordu; artık liste yerinde kalıp yalnızca `[✓]`/`[~]%`/`[✗]` göstergeleri yerinde güncelleniyor.
- `/api/cancel`: `rgsx_manager.py` içinde `_handle_cancel_worker` override'ı — kuyruktan `pop`/`_process_queued_download` yaymadan `request_cancel(task_id)` ile senkronize iptal; iptal edilen indirme akmaz, diğer indirmeler kuyrukta etkilenmez.

**Doğrulanan davranışlar (manuel testler):**
- Fallback: `RGSX_NO_MANAGER=1` ve `--ui-only` → `ensure_manager()` `False` (yerel kuyruk), manager sağlıklıyken `True` (HTTP delege). Yerel modda `start_or_queue_download` manager'a delege etmeden yerel `download_rom` görevi başlatır.
- Auto-start: `--auto-start-install` → `HKCU\...\Run\RGSXManager` değeri `pythonw.exe ...\rgsx_manager.py --minimized` yazılır; `--auto-start-remove` → değer silinir.
- Tray: 5 menü öğesi (Open Web UI / Downloads folder / Logs folder / Auto-start on boot / Exit); Web UI `localhost:port`, klasörler `startfile`, auto-start toggle Registry'yi çevirir, Exit `_trigger_shutdown` (STOP + temiz kapanış). Eksik klasörde notify gösterir, çökmez.
- İptal: yavaş sunucudan 2 indirme (2 slot) → `/api/cancel` ile biri iptal (aktarım durur), diğeri kuyrukta devam eder; deadlock/yarış yok.
- Linux/Batocera: shim ve manager cross-platform — üst seviyede Windows-only import yok (`winreg`/`pystray` fonksiyon içinde), tray/auto-start Linux'ta zarifçe devre dışı kalır, port serbest bırakma `lsof`+`kill` ile çalışır, Batocera servisi `batocera-services` ile kurulur.

---

## Orijinal RGSX Özellikleri

### v2.6.4.9

- Akıllı Sistem Tespiti (`es_systems.cfg` otomatik okuma)
- Akıllı Arşiv Yönetimi (ZIP desteklenmiyorsa otomatik çıkarma)
- Premium Kaynak Desteği (1Fichier API + AllDebrid/Debrid-Link/Real-Debrid/TorBox)
- Özelleştirilebilir Arayüz (3×3 - 4×4 layout, fontlar, diller)
- Kontrolcü Desteği (otomatik eşleme + özel yeniden eşleme)
- Gelişmiş Filtreleme (isme göre arama, platform filtreleme)
- İndirme Yönetimi (kuyruk, geçmiş, ilerleme bildirimleri)
- Erişilebilirlik (ayrı font ölçekleme, klavye modu)
- Web Arayüzü (Batocera/Knulli - uzaktan indirme)
- Arka Plan Müzik Desteği
- Symlink/Copy Seçenekleri
- Otomatik Güncelleme

---

## Gelecek Planlar (Roadmap)

### v2.6.5.0 - Arka Plan İndirme Servisi

**Problem:** TV UI (Pygame) ve indirme motoru aynı process içinde. TV UI kapatıldığında tüm indirmeler öldürülüyor.

**Hedef:** TV UI kapatılsa bile indirmeler arka planda devam etsin.

**Mimari:** Service/Worker Pattern
- `rgsx_service.py`: Bağımsız indirme servisi (daemon)
- REST API (localhost:6999/api/downloads)
- TV UI ve Web UI istemci olarak bağlanır

**Durum:** [x] Tasarım tamamlandı, [x] Uygulandı (bkz. yukarıdaki "RGSX Download Manager (v2.6.5.2)" bölümü)
