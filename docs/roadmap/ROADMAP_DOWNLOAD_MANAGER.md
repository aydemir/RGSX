> Görevler tasks/ klasörüne taşındı, bkz. tasks/todo, tasks/in-progress, tasks/gap. Bu dosya artık sadece tarihsel referans.

# RGSX Fork — Download Manager Yol Haritası (Revize)

Önceki `ROADMAP.md` (Faz 1-7) tamamlandı; bu belge **fork'a özgü** yeni dönem yol
haritasıdır. Kapsam: Windows/RetroBat background download daemon (`rgsx_manager.py`)
ve embedded torrent backend (`qbittorrent_backend.py`). Tüm fazlar bağımsız commit
olarak uygulanır, doğrulanır ve push edilir.

> Revizyon gerekçesi: 2026-08-10 tarihli kod incelemesinin bulguları koda karşı
> doğrulandı (aşağıda). Roadmap, doğrulanmış gerçeklere göre yeniden yazıldı.

---

## Doğrulama Sonuçları (2026-08-10)

Bulguların koda karşı kontrolü — yanlış olan iddialar düzeltildi, roadmap bu halleriyle yazıldı:

| İddia | Sonuç | Kanıt |
|---|---|---|
| Support ZIP secret sızıntısı | ✅ **Doğrulandı** | `utils.py:1392` `rgsx_settings.json`'ı redaksiyonsuz paketliyor |
| API key'ler de sızıyor | ⚠️ **Düzeltildi** | API key'ler ayrı dosyalarda (`utils.py:4290-4296`), zip'e girmiyor — sızıntı qBittorrent şifresiyle sınırlı |
| Firewall marker doğrulama-öncesi yazılıyor | ✅ **Doğrulandı** | `RGSX Retrobat.bat:508` marker'ı `:510`'daki script'ten önce |
| qBittorrent portu hardcoded | ✅ **Doğrulandı** | `qbittorrent_backend.py:28` `_TARGET_PORT=18572`; Windows'ta fallback yok, Linux `_find_free_webui_port()` kullanıyor |
| Watchdog yok | ✅ **Doğrulandı** | `manager_healthy()` (`rgsx_manager.py:793`) tek seferlik; sürekli poll yok |
| Test coverage teatral | ⚠️ **Kısmen eskimiş** | `.coveragerc` kritik dosyaları omit ediyor (doğru), **ama** tests/ artık 8 dosya, 151 test — "2 dosya" iddiası geçersiz |
| Download durum modeli 3 durum | ⚠️ **Kısmen yanlış** | `Queued/Paused/Connecting/Extracting/Converting/Seeding` zaten var; eksik olan transient/permanent ayrımı + retry |
| Yusuf rastgele şifre üretimi ekledi | ❌ **DRIFT — kodda yok** | `secrets`/`urandom`/`generate_random_password` hiçbir yerde geçmiyor; settings'te alan yoksa config sabiti (`RGSXqbt`) kullanılıyor |

**DRIFT detayı (Faz 5'i zorunlu kılıyor):** "Şifre Değiştir" mekanizması gerçekten var
(`change_webui_password`, `qbittorrent_backend.py:956`), ama **zorunlu rastgele şifre
üretimi uygulanmamış**. `KNOWN_DEFAULT_PASSWORDS` ve `migration_v1` flag de yok.
Bölüm 0'daki migration iş akışı hâlâ öneri aşamasında.

---

## Faz 1 — P0: Support ZIP secret redaksiyonu ✅ TAMAMLANDI

**Amaç:** `generate_support_zip()` destek ZIP'ine `rgsx_settings.json`'ı redakte ederek eklemek.

**Neden?** Doğrulanmış açık: ZIP, kullanıcının qBittorrent WebUI şifresini düz metin
içeriyor. Kod bunu çözmek yerine yalnızca `README.txt` içine "DO NOT share this file
publicly" uyarısı koyarak sorumluluğu kullanıcıya devrediyor.

**Kapsam:** Redaksiyon **yalnızca bellekteki kopya** üzerinde yapılır; diskteki orijinal
`rgsx_settings.json` asla değiştirilmez.

**Uygulama:** `network.py:614`'teki `_redact_headers()` deseni `rgsx_settings.json` için
uyarlanır. `generate_support_zip()`'de `rgsx_settings.json` için `zipf.write()` yerine:
- dosyayı oku → `json.loads`
- kırmızı liste: `qbittorrent_webui_password` (mevcut + gelecekteki tüm `*password*`,
  `*key*`, `*token*`, `*secret*` alanları) → `<redacted>`
- `json.dumps` → `zipf.writestr('rgsx_settings.json', redacted_json)`

API key'ler ayrı dosyalarda olduğundan ayrıca işlem gerektirmez.

**Dosyalar:** `utils.py` (`generate_support_zip`), `qbittorrent_backend.py` (şifre alan adı sabiti).

**Doğrulama:** Sızdırılan alanların `<redacted>` olduğunu kontrol eden test; diskteki
orijinal dosyanın byte-birebir değişmediğini assert eden test.

---

## Faz 2 — Firewall marker doğrulama-öncesi yazılmaz ✅ TAMAMLANDI

**Amaç:** Marker yalnızca probe başarılı olduğunda (`FIREWALL_VERIFIED`) yazılsın.

**Neden?** `RGSX Retrobat.bat:508` marker'ı PowerShell script'i (`:510`) çalıştırılmadan
**önce** yazıyor. Script başarısız olsa bile bir daha denenmiyor — sessiz başarısızlık.

**Kapsam:** `windows/` altındaki marker yazım sırası; script başarı/başarısızlık sinyali vermeli.

**Uygulama (seçenek 1 uygulandı):** Marker yazma sorumluluğu **script'e** devredildi:
1. `rgsx_firewall_setup.ps1` sonunda `Add-RgsxFirewallRule`/`Add-RgsxPortRule` artık
   `$true/$false` döndürüyor (kural mevcut veya başarıyla eklendi). Her iki kural da
   doğrulandığında `Write-FirewallMarker` marker'ı yazıp `exit 0`; aksi halde `exit 1`
   (marker yok → bir sonraki lansmanda yeniden denenir).
2. `-MarkerFile` parametresi elevation'dan geçiyor (UAC refünde `exit 1`, marker yok).
3. `RGSX Retrobat.bat`: script-öncesi marker yazımı + marker dizini mkdir kaldırıldı;
   script `-MarkerFile "%FIREWALL_MARKER%"` ile çağrılıyor.

**Dosyalar:** `windows/RGSX Retrobat.bat`, `windows/scripts/rgsx_firewall_setup.ps1`.

**Doğrulama:** Script kural ekleyemezse marker oluşmaz (exit 1); iki kural da mevcutsa
marker yazılır (exit 0). PowerShell/Windows yokluğundan sandbox'ta canlı test yapılamadı;
sözdizimi parantez denge kontrolü + mantık incelemesi ile doğrulandı.

---

## Faz 3 — qBittorrent WebUI port fallback (Windows) ✅ TAMAMLANDI

**Amaç:** 18572 doluysa torrent backend'inin alternatif porta geçmesi (Linux davranışıyla aynı).

**Neden?** Doğrulandı: `_TARGET_PORT=18572` hardcoded, `_ensure_qbittorrent_running()`
içinde `for candidate_port in [_TARGET_PORT]` tek aday (`qbittorrent_backend.py:501`).
Windows'ta çakışma durumunda backend sessizce başarısız olabilir; Linux'ta
`_find_free_webui_port()` çağrılıyordu ama **stub'tı** (her zaman `_TARGET_PORT` döndürüyordu) —
gerçek fallback iki platformda da yoktu.

**Uygulama:**
- `qbittorrent_backend.py`: `_is_port_free` (bind testi, SO_REUSEADDR'sız) + `_find_available_port`
  (preferred+N aralığı) + `_find_free_webui_port()` artık gerçek port seçiyor; `_PORT_MAX_ATTEMPTS=100`.
- `_ensure_qbittorrent_running()`: her iki platformda `webui_port = _find_free_webui_port()`;
  `0` dönerse net hata + `None`. `_preseed_windows_profile(webui_port)` seçilen portu
  qBittorrent.ini'ye yazıyor (Windows). Yeniden kullanım probe'u `_webui_port_candidates()`
  ile tüm fallback aralığını tarıyor (kapalı portlarda hızlı TCP pre-check ile anında elenir;
  yoksa `_wait_for_webui` kapalı portta 3 sn beklediği için 101 aday = ~5 dk olurdu).
- `get_webui_url()`/`_current_webui_port()`: WebUI adresi fallback portu yansıtıyor;
  `/api/qbittorrent/start` ve password-status `webui_url`'i artık doğru portu döndürüyor.
  Web UI (app.js) hardcoded `18572` yerine response'taki `url`'i kullanıyor.
- `rgsx_manager.py`: yerel `_is_port_free`/`_find_available_port` dublikasyonu kaldırıldı,
  `qbittorrent_backend`'e devredildi (tek ortak implementasyon).

**Dosyalar:** `qbittorrent_backend.py`, `rgsx_manager.py`, `static/js/app.js`,
`tests/test_qbittorrent_port.py` (13 test).

**Doğrulama:** 18572'yi işgal edip backend'in alternatif porta geçtiğini ve indirmenin
çalıştığını gösteren canlı test Windows makinesinde yapılmalı. Sandbox'ta saf port seçim
mantığı 13 testle doğrulandı (target boşken target, doluysa 18572+N, aralık tükenince 0,
probe pre-check, preseed ini yazımı).

---

## Faz 4 — Watchdog / auto-restart ✅ TAMAMLANDI

**Amaç:** `/api/health`'i periyodik poll eden, hysteresis'li watchdog thread; process çökünce otomatik restart.

**Neden?** Doğrulandı: `manager_healthy()` tek seferlik kontrol; sürekli poll eden thread yok.
Process çalışırken çökerse indirmeler sessizce takılı kalır.

**Kapsam — iki seviye:**
1. **Manager process:** `INIT → RUNNING ⇄ DEGRADED → UNRESPONSIVE → RESTARTING → CRASHED`
   (roadmap'teki PORT_RESOLVING/FIREWALL_CHECK/SUBSYSTEMS_STARTING ara durumları işletim
   sırasında hiç loglanmadığı için sadeleştirildi — gerçek başlangıç doğrudan INIT→RUNNING).
2. **qBittorrent backend:** `STOPPED → STARTING → PORT_RESOLVING → WEBUI_AUTH_WAIT →
   RUNNING ⇄ UNRESPONSIVE → RESTARTING`

**Uygulama:**
- **`watchdog.py`** (YENİ, saf/bağımlılıksız): `HysteresisMonitor` (ardışık fail →
  DEGRADED → UNRESPONSIVE; her başarı counter'ı sıfırlar → RUNNING) + `RestartLimiter`
  (kayan pencerede max restart, crash-loop önler). İki seviye de aynı modülü kullanır.
- **`rgsx_manager.py`**: `_start_watchdog()` thread'i her 5 sn `/api/health` poll eder;
  3 ardışık FAIL → `DEGRADED`, 6 → `UNRESPONSIVE`; ardından `_spawn_manager()` ile aynı
  argümanlarla spawn + mevcut süreci kapatır (`_restart_manager_for_settings()` deseni
  ortak `_spawn_manager()` yardımcısına refactor edildi, `--no-tray` dahil orijinal
  argümanları korur). Restart limiti aşılırsa `CRASHED` + log (dış supervisor'a devreder).
  `/api/health` artık `manager_state` döndürür.
- **Dış supervisor — TV UI (`__main__.py`)**: roadmap'in "tray supervisor" fikri fiziksel
  olarak imkânsızdı (tray, manager process'inin İÇİNDE yaşar → manager çökerse tray de ölür,
  supervise edemez). Gerçek dış supervisor manager'ı spawn eden **TV UI sürecidir**:
  `_manager_supervisor_loop` 5 sn'de bir health poll; UNRESPONSIVE → `_spawn_manager_process()`
  ile respawn + `_wait_for_manager_ready()` (port fallback'e kayarsa settings'ten yeniden okur).
  Restart limiti aşılırsa CRASHED log'u. TV UI kapalıyken daemon-only kurulumlarda alternatif:
  **Task Scheduler (Windows)** / **systemd unit (Linux)** ile `rgsx_manager.py --minimized`
  auto-restart.
- **qBittorrent (`qbittorrent_backend.py`)**: `_ensure_qbittorrent_running()` yaşayan ama
  WebUI'su yanıt vermeyen process için sınırlı retry (`_WEBUI_RESPONSIVE_RETRIES=3`); tükenince
  UNRESPONSIVE → `_terminate_managed_process()` → probe/taze başlatma akışına düşer
  (RESTARTING→RUNNING). Durum takibi: `get_backend_state()` + `_set_qbt_state()`.

**Dosyalar:** `watchdog.py` (YENİ), `rgsx_manager.py`, `__main__.py`, `qbittorrent_backend.py`.

**Doğrulama:** `tests/test_watchdog.py` (12 test, watchdog.py %100 kapsam; sandbox). Canlı dev
makinesinde: (1) manager PID'ini kill → TV UI supervisor'ın respawn ettiğini spawn log'dan doğrula;
(2) qBittorrent process'ini kill → backend log'unda UNRESPONSIVE→RESTARTING→RUNNING geçişlerini
gözlemle.

---

## Faz 5 — qBittorrent şifre migration v1 (DRIFT düzeltmesi) ✅ TAMAMLANDI

**Amaç:** Öntanımlı şifrede kalan kurulumları tek seferlik otomatik migration ile rastgele
şifreye taşımak; kullanıcı tanımlı şifreye **asla dokunmamak**.

**Neden?** Doğrulanmış DRIFT: rastgele şifre üretimi kodda yok, kurulumlar öntanımlı
`RGSXqbt`'de duruyor. Bu, Faz 1'deki P0 ile aynı güvenlik sınıfı.

**Uygulama — her başlatmada, qBittorrent ilk RUNNING olduğunda:**
```
stored_password = get_qbittorrent_webui_password()  # settings'te alan yoksa None dönecek şekilde ayrıştır

eğer stored_password is None:               # hiç kurulmamış
    extracted = _extract_temp_password()    # qBittorrent log'undan
    yeni = extracted or generate_random_password()   # secrets.token_urlsafe, kriptografik
    settings + qBittorrent'e uygula; TVUI bildirim; DUR

eğer stored_password in KNOWN_DEFAULT_PASSWORDS:    # hâlâ öntanımlıda
    yeni = generate_random_password()
    settings + qBittorrent'e uygula
    tek seferlik TVUI bildirim (migration_v1_done flag); DUR

aksi halde:                              # kullanıcı değiştirmiş
    hiçbir şey yapma; stored_password'ü aynen uygula
```

**Kritik guard'lar:**
1. `KNOWN_DEFAULT_PASSWORDS` sabit liste: qBittorrent bilinen varsayılanları + eski hardcoded
   `RGSXqbt`. `_TEMP_PASSWORD_PATTERNS` **bu listeye dahil edilmez** (geçici şifre zaten rastgele).
2. `migration_v1_done` flag `rgsx_settings.json`'da — migration **bir kereliğine** çalışır.
   Flag olmadan, kullanıcı ileride bilinçli olarak öntanımlıya dönerse her başlatmada üzerine
   yazılır.
3. `get_qbittorrent_webui_password`'ün mevcut davranışı (alan yoksa config sabitine düşme)
   migration'da kullanılamaz; "alan yok" ile "alan = öntanımlı" ayrımı için settings'e **doğrudan**
   erişim gerekir (`settings.get("qbittorrent_webui_password")`).

**Dosyalar:** `qbittorrent_backend.py` (`generate_random_password`, migration tetikleyici),
`rgsx_settings.py` (`migration_v1_done` persister), `rgsx_manager.py`/`__main__.py` (başlangıç
hook), TVUI bildirim çevirileri.

**Doğrulama:** ✅ `tests/test_password_migration.py` (21 test) — üç senaryo (a) alan yok →
rastgele üretilir, (b) alan = öntanımlı → rastgele üretilir + flag yazılır, (c) alan = kullanıcı
tanımlı → dokunulmaz; ikinci başlatmada flag nedeniyle hiçbir şey yapılmaz (`already_done`).
Toplam 140 test geçti, py_compile temiz. Canlı dev makinesinde: şifresiz/`RGSXqbt`'li kurulumda
ilk RUNNING sonrası settings'te rastgele şifre + `migration_v1_done: true`; ikinci başlatmada
"already_done" log'u.

---
## Faz 6 — Büyük dosya refaktörü: tekil .py → paket (display.py deseni) ✅ TAMAMLANDI (6-1..6-5)

**Amaç:** 2000+ satırlık tekil dosyaları display.py deseniyle (8f094aa: 6818 satır → `display/`
paketi, `__init__.py` public API re-export, davranış değişmez) paketlere bölmek. Import yüzeyi
ve modül-seviyesi state korunur; davranış değişmez.

**Neden?** Yeni fazların (state modeli, toplu indirme, Rust) hedefi bu dosyalar; tekil
God Object'ler değişimi riskli yapıyor. Mevcut durum (2026-08-10 ölçümü):

| Dosya | Satır | Kilit nokta |
|---|---|---|
| `network.py` | 5667 | indirme hattı + modül-seviyesi state (`progress_queues`, `cancel_events`, `pause_events`, `download_threads`) — `thread_safety.py`, `rgsx_cli.py` doğrudan import ediyor |
| `controls.py` | 4970 | `handle_controls` **3626 satır** (`:1240-4865`), tek if-elifs zinciri; `language.py` `VALID_STATES` import ediyor |
| ~~`utils.py`~~ → `utils/` paketi | 4776 → 12 modül | ✅ Faz 6-1'de bölündü; en geniş fan-in: network, controls, rgsx_web, rgsx_manager, __main__, history, rgsx_cli, qbittorrent_backend |
| `rgsx_web.py` | 2408 | `RGSXHandler` **1759 satır** (`:464-2223`); `rgsx_manager.py` `import rgsx_web` + `RGSXHandler/get_cached_games/get_translation` |
| `__main__.py` | 2106 | TVUI entry + manager spawn/supervisor karışık |

**Alt fazlar (her biri bağımsız commit — aynı desen, aynı doğrulama):**

- **Faz 6-1 — `utils.py` → `utils/` paketi.** ✅ TAMAMLANDI (114/114 fonksiyon, sıfır kayıp).
  En geniş fan-in, en düşük risk (saf yardımcılar):
  `games.py` (load_sources/load_games + platform game count cache + `_refresh_loading_feedback`),
  `sorting.py`, `media.py` (badges/ikon + müzik), `torrent.py`
  (manifest cache + bencode + URL parse), `services.py` (web/DNS boot + connection status +
  restart), `extensions.py` (ES systems), `text.py` (truncate/sanitize/wrap +
  `_format_size_bytes`/`get_clean_display_name`), `extract.py` (zip/rar/7z +
  ps3/dos/scummvm/psvita/xbox handler'ları + `_resolve_7z_command`), `security.py` (redact +
  support zip), `api_keys.py`, `history_matches.py`, `files.py` (disk/klasör/arama + `DiskUsage`).
  `__init__.py` tüm public isimleri re-export eder. Döngü çözümü: `games.py` ↔ `torrent.py`
  arasında `_refresh_loading_feedback` lazy import (torrent içinde fonksiyon-seviyesinde);
  urllib3/requests log susturma `__init__.py`'de; `logger` kimliği (`logging.getLogger("utils")`)
  tüm alt modüllerde korunur.
- **Faz 6-2 — `network.py` → `network/` paketi.** ✅ TAMAMLANDI (94/94 fonksiyon AST diff'te
  birebir — 3 lazy import dışında). Kritik: modül-seviyesi state
  (`progress_queues`, `cancel_events`, `pause_events`, `download_threads`, `torrent_temp_roots`,
  `_app_shutting_down`, `urls_in_progress`, `urls_lock`, `url_results`, `url_done_events`)
  `network/__init__.py`'de **aynı obje kimliğiyle** tutulur —
  `thread_safety.py`'deki `from network import pause_events` vs. aynı çalışmaya devam eder.
  Modüller: `upnp.py` (UPnP + aria2/torrent seeding), `http_download.py`
  (headers/challenge/resume/vimm/browser), `lolroms.py`, `archive_org.py`,
  `one_fichier.py` (roadmap'teki `1fichier.py` — Python leading-digit modül adıyla
  `from network.1fichier import` çalışmadığı için yeniden adlandırıldı), `queue.py`
  (worker + pause/resume/cancel/shutdown + state + `download_rom`), `updates.py`
  (changelog + extract_update), `helpers.py`. Döngü kırma (lazy import, utils deseni):
  helpers↔http_download (`_build_browser_download_headers`), queue↔one_fichier
  (`download_queue_worker` içinde). **Pre-existing NameError fix:** eski `network.py`'de
  `logger` 419 kez kullanılıp hiç tanımlanmamıştı → `logging.getLogger("network")` tanımlandı
  (utils deseni); aynı şekilde `InsufficientDiskSpaceError` raise/except'te kullanılıyor ama
  hiç tanımlanmamıştı → `helpers.py`'de sınıf eklendi + re-export.
- **Faz 6-3 — `rgsx_web.py` → `rgsx_web/` paketi.** ✅ TAMAMLANDI (2408 satır; 15/15 metot AST
  diff birebir, do_GET 13 + do_POST 10 branch tek tek inline elif gövdeleriyle eşit). Yapı:
  `__init__.py` (logging bootstrap — `FlushFileHandler` + rotation + crash log + console —,
  ilk veri yükleme, cache/i18n/handler/server re-export, shim), `cache.py` (etag/
  cached_games/invalidation/watchdog — aynı obje kimliği), `i18n.py` (translations +
  normalize_size), `handlers.py` (`RGSXHandler(UIMixin, GamesMixin, DownloadMixin,
  SettingsMixin, BaseHTTPRequestHandler)` + dispatcher + ortak yanıtlar), `handlers_ui.py`,
  `handlers_games.py`, `handlers_download.py`, `handlers_settings.py`, `server.py`
  (run_server + CURRENT_HTTPD). Sözleşme `import rgsx_web` +
  `RGSXHandler/get_cached_games/get_translation/run_server/CURRENT_HTTPD` korundu.
  **Düzeltmeler:** `handlers_qbittorrent.py` **gereksiz** — orijinal `rgsx_web.py`'de
  qBittorrent handler'ı hiç yoktu (`grep -c qbt` = 0); endpoint'ler Faz 3/5'te zaten
  `rgsx_manager.py` `ManagerHandler`'ına gitti. `FlushFileHandler` `server.py`'ye taşınamadı
  (logging bootstrap'ı `logger`'dan önce çalışır, `server.py` `from . import logger` ister →
  döngüsel import); `__init__.py`'de kaldı. `_serve_static_file`/`_asset_version`'da
  `Path(__file__).parent` → `config.APP_FOLDER` (paket içinde __file__ rgsx_web/ dizini olur,
  eski çözünürlük bozulurdu; APP_FOLDER eşdeğeri).
  **Doğrulama:** pytest `tests/ -q` → 183 passed / 23 pre-existing display (HEAD baseline
  birebir); live smoke tüm endpoint'ler doğru JSON; `rgsx_manager` import + ManagerHandler MRO
  + `super().do_GET()` geçişi OK.
- **Faz 6-4 — `controls.py` → `controls/` paketi.** ✅ TAMAMLANDI (4970 satır, 54 item,
  5 modül; 54/54 AST diff birebir — 1 lazy import dışında). Yapı:
  `input.py` (is_input_matched + key state + joystick), `menus.py` (folder browser + filter
  menus + `VALID_STATES`/`validate_menu_state`), `downloads.py` (start_or_queue_download +
  kuyruk + delegate), `search.py` (global search), `handlers.py` (handle_controls dispatch).
  `__init__.py` re-export ile modül-state (`key_states`/`_platform_torrent_support_cache`)
  **aynı obje kimliğiyle** korunur; logger kimliği `controls` korunur. Döngü kırma (lazy
  import, utils deseni): `input.py`↔`handlers.py` `process_key_repeats` içinde. Fix:
  `_platform_torrent_support_cache` AnnAssign yakalama düzeltmesi. `language.py`'deki
  `from controls import VALID_STATES` ve `__main__.py` import yüzeyi birebir korunur.
  **Doğrulama:** tam suite baseline ile aynı (184 passed / 22 pre-existing display+pygame-stub);
  dev makinesinde TVUI + WebUI live smoke testi yapıldı.
- **Faz 6-5 — `__main__.py` inceltme.** ✅ TAMAMLANDI — boot + `main()` `tvui.py`'ye,
  manager spawn/supervisor `manager_launcher.py`'ye taşındı (`ensure_manager`/
  `_start_manager_supervisor` watchdog tabanlı); `__main__.py` yalnız DPI + logging
  bootstrap + dispatch (`from tvui import main`). DPI çağrısı config'ten önce korundu;
  `python __main__.py` aynı davranışı sürdürür. `.gitignore`'a yeni paket `__pycache__`
  klasörleri eklendi. **Doğrulama:** baseline aynı (183 passed / 23 pre-existing display+pygame-stub).

**Her alt fazın doğrulaması:**
- `git mv` ile taşı (history korunur); yeni `__init__.py` re-export; `python -m py_compile`
  temiz; tüm `from X import Y` çağrılarının bozulmadığı grep ile doğrulanır.
- `RGSX_HEADLESS=1 PYTHONPATH=/tmp/pygame_stub python -m pytest tests/ -q` tam geçer.
- Modül-seviyesi state kimlikleri korunur (network paketi: `tests/test_thread_safety.py`).
- Canlı dev makinesinde TVUI + WebUI smoke testi (Faz 6-4 için şart) — ✅ yapıldı.

---

## Faz 7 — Test altyapısı: characterization tests (Rust önkoşulu) ✅ TAMAMLANDI

**Amaç:** Mevcut `/api/*` ve SSE davranışını "altın standart" olarak sabitleyen test seti.

**Neden?** `.coveragerc` kritik dosyaları omit ediyor; `network.py`/`rgsx_web.py`/`rgsx_manager.py`/
`qbittorrent_backend.py` için davranışsal test yok. Rewrite sırasında hangi davranışın kasıtlı
hangi davranışın kaza olduğu ayırt edilemez.

**Uygulama:**
- Endpoint envanteri: `rgsx_web` paketi (~24 endpoint) + `rgsx_manager.py` (10 endpoint) çıkarıldı.
- Response şekli, hata kodları, SSE event sırası (`snapshot/progress/history/queue/downloaded`)
  için request-level testler — handler'lar gerçek soket olmadan (`object.__new__` + mock
  wfile/rfile/headers) doğrudan çağrılarak.
- `.coveragerc`'ten kritik dosyaların omit'i tek tek kaldırıldı
  (öncelik: `qbittorrent_backend.py` → `rgsx_settings.py` → `rgsx_manager.py`).
- Ölü (eski monolit) omit girişleri temizlendi: `network.py`/`rgsx_web.py`/`controls.py`/
  `utils.py` artık paket olduğundan hiçbir şey eşleşmiyordu; `*/network/*` + `*/utils/*` ile
  değiştirildi. `rgsx_web/` paketi ve üç öncelikli modül artık ÖLÇÜLÜYOR.

**Dosyalar:** `tests/test_api_contract.py` (52 test), `tests/test_qbittorrent_backend.py`
(11 test), `.coveragerc`.

**Kapsam düşüşü (belgelendi — omit kaldırmanın kasıtlı sonucu):** bu dosyalar önceden
omit'liydi (ölçülen kapsam yok). Faz 7 sonrası ilk kez gerçek sayılarda:

| Dosya | Kapsam |
|---|---|
| `rgsx_web/handlers.py` | 76% |
| `rgsx_web/handlers_settings.py` | 62% |
| `rgsx_web/handlers_games.py` | 58% |
| `rgsx_web/__init__.py` + `cache.py` | 55% |
| `rgsx_web/handlers_ui.py` | 48% |
| `rgsx_web/handlers_download.py` | 36% |
| `rgsx_web/i18n.py` | 30% |
| `rgsx_web/server.py` | 10% |
| `rgsx_manager.py` | 31% |
| `rgsx_settings.py` | 23% |
| `qbittorrent_backend.py` | 20% |
| **TOTAL** | 18% (9538 stmt ölçülüyor) |

**Doğrulama:** Baseline korundu — tam suite **246 passed / 23 failed** (183 öncesi + 63 yeni
contract testi; 23 hata pre-existing display/pygame-stub kaynaklı, değişmedi). `rgsx_web/*`
dosya bazlı ölçümlerin kapsamına `rgsx_settings.py`/`rgsx_manager.py`/`qbittorrent_backend.py`
için hedef bir sonraki Faz 7 iterasyonunda daha fazla testle artırılacak.

---

## Faz 8 — Download item state modeli genişletmesi ✅ TAMAMLANDI

**Amaç:** Mevcut durumlara transient/permanent hata ayrımı ve retry eklemek; sözlük yerine açık model.

**Neden?** Doğrulandı: `Queued/Paused/Connecting/Extracting/Converting/Seeding` zaten var, ama
hata tek "Failed" gibi ele alınıyor; retry yok. Toplu indirmede (Faz 9) rate limit / geçici ağ
kesintisi kaçınılmaz — bu faz Faz 9'un **önkoşulu**.

**Uygulama:**
- **State** (anlık): `DOWNLOADING`, `PAUSED`, `FAILED_TRANSIENT`, `FAILED_PERMANENT`,
  `RETRY_SCHEDULED`, `VERIFYING`, `EXTRACTING`.
- **Event** (tetikleyici): `PAUSE_REQUESTED`, `RETRY_TRIGGERED`, `PERMANENT_FAILURE`...
- **Transition** (yan etkili): `DOWNLOADING + PAUSE_REQUESTED → PAUSED`
  (`downloader.pause() + persist_state() + emit_event()`).
- Minimal `@dataclass DownloadJob(id, url, destination, state, progress, retry_count, error)`
  serbest sözlük kullanımının yerine; `history.json`'a yazım geriye dönük uyumlu kalır
  (mevcut alan adları korunur, ek alanlar eklenir).
- Retry: `FAILED_TRANSIENT → RETRY_SCHEDULED → DOWNLOADING` (backoff, `retry_count`).

**Dosyalar:** `network.py` (job modeli), `history.py` (persistence), `rgsx_web.py` (SSE event'leri),
`display/` (UI durum ikonları).

### Uygulama notları (koda karşı doğrulandı)

- **`network/` monoliti artık paket** olduğundan "network.py (job modeli)" → yeni
  **`network/download_state.py`** modülü (saf/bağımsız): `DownloadState`/`DownloadEvent`
  enum'ları, `_TRANSITIONS` tablosu, `transition(job, event, effects)` yan-etkili geçişi
  (geçersiz kombinasyonda `IllegalTransitionError`), `DownloadJob` @dataclass
  (`from_history_entry`/`apply_to_history_entry` — eski format geriye dönük uyumlu,
  status/url/... mevcut alan adları korunur, `entity_state`/`retry_count`/`max_retries`/
  `error`/`retry_at` eklenir), `classify_error` (transient: 408/409/425/429/5xx + timeout +
  connection + bariz transient marker'lar; permanent: 401-4xx + paylaşılan kalıcı
  marker'lar + `InsufficientDiskSpaceError`; belirsiz → varsayılan kalıcı), üstel backoff
  (`5s,10s,20s,... max 300s`), legacy `<->` enum eşlemesi, opsiyonel SSE emitör
  (`set_state_emitter`).
- **`network/queue.py`**: `download_rom`'un ana döngü ve drain sonuç blokları artık
  `_finalize_download_result(task_id, url, success, message, platform, game_name, entry)`'ye
  yönlendiriyor → success `COMPLETED`/`Download_OK` (mevcut davranış aynı); transient &
  `retry_count < max` → `FAILED_TRANSIENT → RETRY_SCHEDULED` (status `Téléchargement`'te kalır,
  aktif görünüm, `message`'a retry metni, `retry_at`), `_schedule_download_retry` backoff
  sonrası yeni task_id ile `download_rom`/`download_from_1fichier`'ı yeniden başlatır
  (slots beklenir, `_retry_in_flight` dublikasyonu önler, iptal/kapanışta atlanır); kalıcı →
  `FAILED_PERMANENT`/`Erreur` (mevcut davranış aynı). `download_rom` başlangıcında
  `entity_state = DOWNLOADING` sıfırlanır.
- **`network/one_fichier.py`**: aynı iki sonuç bloğu + başlangıç reset'i aynı modele bağlandı
  (lazy import ile döngü kırıldı).
- **`history.py`**: yazım zaten lenient (ek alanlar korunur); eski format okuma regression'ı
  test edildi — değişiklik gerekmedi.
- **SSE**: `rgsx_manager.main()` `set_state_emitter(_broadcast)` kaydeder → durum değişimleri
  `download_state` SSE event tipiyle yayınlanır; ek olarak `entity_state`/`retry_*` alanları
  `config.history` repr'ini değiştirdiği için mevcut `history` SSE diff'i de taze state'i taşır.
- **`display/history.py`**: `RETRY_SCHEDULED`/`FAILED_TRANSIENT` için status sütunu
  (`history_status_retrying` çevirisi: "Retry {0}/{1}") + uyarı rengi eklendi.
- **Pre-existing bug fix:** `_app_shutting_down` flag'i `queue.py`'de kötü bir şekilde ayrı
  modül kopyası olarak set ediliyordu (kimse okumuyordu) — artık paylaşılan
  `network._app_shutting_down` set ediliyor; retry runner'ı kapanışı böyle görüyor.
- **Yapılandırma:** `config.DOWNLOAD_MAX_RETRIES=3`,
  `DOWNLOAD_RETRY_BACKOFF_BASE_SEC=5.0`, `DOWNLOAD_RETRY_BACKOFF_MAX_SEC=300.0`.
- **7 dil:** `download_retry_attempt` + `history_status_retrying` anahtarları.

**Doğrulama:** Geçici hata → `retry_count` artar ve tekrar dener; kalıcı hata → `FAILED_PERMANENT`.
`history.json` eski formatla yazılmış veriyi okuyabilir (regression). `tests/test_download_state.py`
57 test (transition/illegal, classifier, backoff, legacy mapping, job round-trip, finalize entegrasyonu,
retry relaunch thread, shutdown abort, emitter, history eski-format regression). Tam suite:
**325 passed / 23 pre-existing display+pygame-stub** (HEAD baseline birebir aynı 23).

---

## Faz 9 — Filtreli listeyi toplu indirme ("Tümünü İndir") ✅ TAMAMLANDI

**Amaç:** Filtrelenmiş listenin tek seferde kuyruğa alınması.

**Neden?** ROM koleksiyonu senaryosu; tek oyunla sınırlıydı. Kapsam kararlaştırılan akış:
checkbox modu yok, listenin **ilk satırında** "Tümünü İndir" satırı; kapsam her zaman o an
ekranda görünen (filtrelenmiş) set; zaten indirilmiş oyunlar zorunlu atlanmaz (kullanıcı
`hide_downloaded` filtresiyle hariç tutar) — yalnızca sayaç sayılır.

**Uygulama (koda karşı doğrulandı):**
- **Web endpoint — `rgsx_web/handlers_download.py:501` `_api_download_batch`** (`POST
  /api/download/batch`, payload `{platform, game_names[]}`): 400 doğrulamaları (platform yok /
  liste boş); oyunlar isim/display_name üzerinden cached katalogdan çözülür; URL dedupe
  (mevcut kuyruk + aynı batch içi `seen_urls`); extension kontrolü; `already_downloaded`
  sayacı (atlama değil, sayıdır); her item mevcut `QUEUED` akışına `batch_*` task_id ile girer;
  `save_history` tek sefer; yanıt `{queued, skipped, already_downloaded, errors}`.
- **Tek tüketici kuralı — `_kick_batch_if_no_worker` (handlers_download.py:628):** `config.
  queue_worker_running` True ise (manager süreci) hiçbir şey yapılmaz — kuyruğun tek tüketicisi
  `download_queue_worker`'dır (aksi halde worker + legacy thread zinciri çift pop ederdi).
  False ise (standalone web) legacy thread zinciri `_process_queued_download`'ı boş slot sayısı
  kadar başlatır.
- **ManagerHandler yönlendirmesi:** `rgsx_web/handlers.py` + `rgsx_manager.py` `_api_download_batch`'i
  çağırır — manager process'te worker zaten çalıştığından yalnızca kuyruğa basar.
- **TV UI çekirdeği — `controls/downloads.py:149` `queue_download_batch(games, platform_label)`:**
  `(queued, skipped, already_downloaded, errors)` döner; `_queue_download(defer_save=True)` her
  öğe için history.json'a yazmaz/toast göstermez — çağıran sonunda tek `save_history` + toplu
  toast gösterir; kuyruk boş değilse `_launch_next_queued_download()`. `trigger_filtered_batch_
  download()` (downloads.py:201) görünen seti (filter_active → `filtered_games`, değilse `games`)
  daemon thread'de kuyruğa alır, UI bloklamaz.
- **TV UI satırı:** `display/game_list.py` filtrelenmiş liste üstüne "Tümünü İndir" satırı +
  `config.download_all_focus` (controls/menus.py, controls/handlers.py) — odak, A tuşu → batch.
- **WebUI:** `static/js/app.js` batch düğmesi → `POST /api/download/batch`.
- **Yapılandırma:** `config.queue_worker_running` (manager True, standalone web False),
  `config.download_all_focus`.
- **7 dil:** `game_download_all_toast` anahtarı.

**Dosyalar:** `rgsx_web/handlers_download.py`, `rgsx_web/handlers.py`, `rgsx_manager.py`,
`controls/downloads.py`, `controls/handlers.py`, `controls/menus.py`, `display/game_list.py`,
`config.py`, `static/js/app.js`, 7 dil JSON'u, `tests/test_download_batch.py`.

**Doğrulama:** `tests/test_download_batch.py` — 16 test (web endpoint: 400'ler, tam batch
kuyruğa girer, bilinmeyen oyun atlanır, URL'siz atlanır, batch-içi dup, kuyrukta dup,
already-downloaded sayacı, worker-running → kick yok, standalone kick boş slotları doldurur;
manager yönlendirme; TV çekirdeği: sayaçlar, unsupported skip, dedupe, async trigger). Tam
suite: **341 passed / 23 pre-existing display+pygame-stub** (baseline 325 + 16 yeni). Canlı
smoke: 200 oyunluk filtrelenmiş listede batch kuyruğa girer; `hide_downloaded` açıkken
indirilmişler listede olmadığından kuyruğa girmez; transient hatalar Faz 8 retry'i ile
yeniden denenir.

---

## Faz 10 — Rust kısmi refaktör (EN SON)

**Amaç:** State machine + concurrency-ağır manager'ı Rust'a taşımak; Linux/Batocera desteği kırılmadan.

**Motivasyon:** `enum`+`match` ile compiler-enforced state transition'ları; `librqbit`
(`rqbit` motoru) embedded qBittorrent'i ikame edebilir. **Kısıt:** bu faz ancak Faz 1-9
tamamlandıktan sonra — özellikle Faz 7 (characterization tests) olmadan başlanamaz.

**Platform bölünmesi (doğrulanmış kısıt):**
| Bileşen | Platform kapsamı | Rust'a geçiş |
|---|---|---|
| `rgsx_manager.py` (daemon, tray, autostart, port resolve, SSE, watchdog) | Windows-only | ✅ Faz 10a — risk düşük |
| `qbittorrent_backend.py` (embedded torrent, `librqbit` adayı) | Windows **+** Linux/Batocera | ✅ Faz 10b — `TorrentBackend` trait + `LibrqbitEngine` (RGSX_TORRENT_ENGINE=librqbit), Python fallback korunur |

**Ara mimari:** Rust manager binary, mevcut `qbittorrent_backend.py`'yi subprocess olarak
çağırmaya devam eder (JSON-RPC veya local HTTP köprüsü). Windows tarafı kademeli Rust'a geçerken
Linux/Batocera Python'da kalır.

**Sözleşme:** `/api/*` ve `/api/events` (SSE) — mevcut davranış birebir korunur; Faz 7'daki
characterization tests bunun garantisidir.

**Stack:** `tokio` + `axum` (HTTP/SSE), `windows-rs` (registry + firewall COM), `serde` (JSON).
Cross-platform genişlerse `cross-rs`/musl toolchain ile ARM cross-compile.

**librqbit embedded engine — kullanım (Faz 10b):**

- `manager-bin` varsayılan olarak **librqbit** (in-process, `manager-torrent::LibrqbitEngine`)
  kullanır (TASK-002h, 2026-08-12 — Windows derlemesi `cargo check --target x86_64-pc-windows-gnu`
  ile doğrulandı, ertelenmiş karar tetiklendi).
- **Python bridge opt-in:** `RGSX_TORRENT_ENGINE=python` → legacy qbittorrent_backend.py
  subprocess (WebUI / port-fallback / şifre migration korunur).
- Diğer env'ler:
  - `RGSX_DOWNLOADS_FOLDER=<dir>` → indirme hedefi (öntanımlı `%TEMP%/rgsx_torrents`).
  - `RGSX_LOGS_FOLDER=<dir>` → log hedefi (öntanımlı `%TEMP%`).
  - `RGSX_MANAGER_BIN_PORT=<port>` → HTTP portu (öntanımlı 5010).
- Doğrulama (2026-08-12, aarch64 Linux): `manager-bin` gerçek bir `.torrent` ile uçtan uca
  indirdi → `POST /api/download` → `finalize_download_in_state` → history `Download_OK`,
  dosya `downloads_folder`'a hard-link ile çıktı. Windows cross-compile (tüm workspace,
  tray/autostart/firewall dahil) `x86_64-pc-windows-gnu` hedefinde hatasız derlendi.

**KARAR GERÇEKLEŞTİ (2026-08-12, TASK-002h):** librqbit **varsayılan torrent motoru** yapıldı;
Python bridge `RGSX_TORRENT_ENGINE=python` ile opt-in. Gerekçe: Windows derlemesi teyit
edildi. Bilinen kayıp: qBittorrent WebUI / port-fallback / şifre migration / seeding durumu
`embedded_mode`'da mevcut değil — bu yüzden Python yolu bilinçli olarak korundu (opt-in).

**Sıralama:** önce state machine (`enum`), sonra downloader mantığı.

---

## Önerilen Sıra

```
Faz 1 (P0 ZIP redaksiyon)
   ↓
Faz 2 (Firewall marker)
   ↓
Faz 3 (qBittorrent port fallback)
   ↓
Faz 4 (Watchdog)
   ↓
Faz 5 (Şifre migration v1)
   ↓
Faz 6 (Büyük dosya refaktörü: 6-1..6-5) ✅
   ↓
Faz 7 (Characterization tests)
   ↓
Faz 8 (Download state modeli)   ← Faz 9'un önkoşulu
   ↓
Faz 9 (Toplu indirme) ✅
   ↓
Faz 10 (Rust refaktör) ✅       ← Faz 1-9 + 10a (Windows) + 10b (librqbit engine) tamamlandı
   ↓
Faz 11 (İlk açılışta dil algılama)
```

Gerekçeler:
- **1 → 2 → 3**: hızlı, düşük riskli, bağımsız düzeltmeler (P0 + ~10 dk'lık işler).
- **4 (watchdog)**: en yüksek değer — crash durumunda sessiz kayıpları önler.
- **5 (migration)**: P0 ile aynı güvenlik sınıfı; Faz 1'de kurulan şifre sabiti üzerine inşa edilir.
- **7 (tests) → 8 (state) → 9 (bulk)**: state modeli bulk'un, characterization tests Rust'ın önkoşulu.
- **10 (Rust)**: en son — ancak davranış sabitlendikten (Faz 7) ve yeni özellikler oturduktan sonra.
  Faz 9 ile birlikte Faz 1-9 tamamlandı — **Faz 10 sıradaki aktif fazdır**.
- **11 (dil algılama)**: bağımsız; tasarımı Faz 6 sırasında tamamlandı, uygulaması istendiğinde.

---

## Faz 11 — İlk Açılışta Sistem Dili Otomatik Algılama (Planlandı)

> Tasarım, `ROADMAP.md`'deki Faz 8 maddesinden taşındı (o dosya tamamlandı olarak işaretlendi).

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
baseline ile aynı kalır (246 passed / 23 pre-existing).
