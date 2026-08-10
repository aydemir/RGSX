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
## Faz 6 — Büyük dosya refaktörü: tekil .py → paket (display.py deseni)

**Amaç:** 2000+ satırlık tekil dosyaları display.py deseniyle (8f094aa: 6818 satır → `display/`
paketi, `__init__.py` public API re-export, davranış değişmez) paketlere bölmek. Import yüzeyi
ve modül-seviyesi state korunur; davranış değişmez.

**Neden?** Yeni fazların (state modeli, toplu indirme, Rust) hedefi bu dosyalar; tekil
God Object'ler değişimi riskli yapıyor. Mevcut durum (2026-08-10 ölçümü):

| Dosya | Satır | Kilit nokta |
|---|---|---|
| `network.py` | 5667 | indirme hattı + modül-seviyesi state (`progress_queues`, `cancel_events`, `pause_events`, `download_threads`) — `thread_safety.py`, `rgsx_cli.py` doğrudan import ediyor |
| `controls.py` | 4970 | `handle_controls` **3626 satır** (`:1240-4865`), tek if-elifs zinciri; `language.py` `VALID_STATES` import ediyor |
| `utils.py` | 4776 | en geniş fan-in: network, controls, rgsx_web, rgsx_manager, __main__, history, rgsx_cli, qbittorrent_backend |
| `rgsx_web.py` | 2408 | `RGSXHandler` **1759 satır** (`:464-2223`); `rgsx_manager.py` `import rgsx_web` + `RGSXHandler/get_cached_games/get_translation` |
| `__main__.py` | 2106 | TVUI entry + manager spawn/supervisor karışık |

**Alt fazlar (her biri bağımsız commit — aynı desen, aynı doğrulama):**

- **Faz 6-1 — `utils.py` → `utils/` paketi.** En geniş fan-in, en düşük risk (saf yardımcılar):
  `games.py` (load_sources/load_games), `sorting.py`, `media.py` (badges/ikon), `torrent.py`
  (manifest cache + bencode + URL parse), `services.py` (web/DNS boot + connection status),
  `extensions.py` (ES systems), `text.py` (truncate/sanitize/wrap), `extract.py` (zip/rar/7z +
  ps3/dos/scummvm/psvita/xbox handler'ları), `security.py` (redact + support zip), `api_keys.py`,
  `history_matches.py`, `files.py`. `__init__.py` tüm public isimleri re-export eder.
- **Faz 6-2 — `network.py` → `network/` paketi.** Kritik: modül-seviyesi state
  (`progress_queues`, `cancel_events`, `pause_events`, `download_threads`, `urls_in_progress`,
  `url_results`, `url_done_events`) `network/__init__.py`'de **aynı obje kimliğiyle** tutulur —
  `thread_safety.py`'deki `from network import pause_events` vs. aynı çalışmaya devam eder.
  Modüller: `upnp.py`, `http_download.py` (headers/challenge/resume/vimm/browser), `lolroms.py`,
  `archive_org.py`, `1fichier.py`, `queue.py` (worker + pause/resume/cancel/shutdown + state),
  `updates.py` (changelog + extract_update), `helpers.py`.
- **Faz 6-3 — `rgsx_web.py` → `rgsx_web/` paketi.** `RGSXHandler` 1759 satırını endpoint
  grubuna göre ayır: `cache.py` (etag/cached_games/invalidation/watchdog), `i18n.py`
  (translations + normalize_size), `handlers_download.py`, `handlers_qbittorrent.py`,
  `handlers_games.py`, `handlers_settings.py`, `server.py` (run_server + FlushFileHandler).
  `import rgsx_web` + `RGSXHandler/get_cached_games/get_translation` sözleşmesi `__init__.py`'de.
- **Faz 6-4 — `controls.py` → `controls/` paketi.** `handle_controls`'ı menü-durumu dispatch'ine
  böl: `input.py` (is_input_matched + key state + joystick), `menus.py` (folder browser + filter
  menus + `VALID_STATES`/`validate_menu_state`), `downloads.py` (start_or_queue_download +
  kuyruk + delegate), `search.py` (global search), `handlers.py` (handle_controls dispatch).
  `language.py`'deki `from controls import VALID_STATES` korunur; `display/controls.py` ile
  ad karışmaz (üst seviye `controls/` ayrı). Pygame bağımlı olduğu için doğrulama dev
  makinesinde (sandbox'ta sadece py_compile + non-display testler).
- **Faz 6-5 — `__main__.py` inceltme.** Entry dosyası kalır; manager spawn/supervisor mantığı
  `manager_launcher.py`'a, TVUI boot akışı ayrı modüle taşınır. `python __main__.py` aynı
  davranışı sürdürür.

**Her alt fazın doğrulaması:**
- `git mv` ile taşı (history korunur); yeni `__init__.py` re-export; `python -m py_compile`
  temiz; tüm `from X import Y` çağrılarının bozulmadığı grep ile doğrulanır.
- `RGSX_HEADLESS=1 PYTHONPATH=/tmp/pygame_stub python -m pytest tests/ -q` tam geçer.
- Modül-seviyesi state kimlikleri korunur (network paketi: `tests/test_thread_safety.py`).
- Canlı dev makinesinde TVUI + WebUI smoke testi (Faz 6-4 için şart).

---

## Faz 7 — Test altyapısı: characterization tests (Rust önkoşulu)

**Amaç:** Mevcut `/api/*` ve SSE davranışını "altın standart" olarak sabitleyen test seti.

**Neden?** `.coveragerc` kritik dosyaları omit ediyor; `network.py`/`rgsx_web.py`/`rgsx_manager.py`/
`qbittorrent_backend.py` için davranışsal test yok. Rewrite sırasında hangi davranışın kasıtlı
hangi davranışın kaza olduğu ayırt edilemez.

**Uygulama:**
- Endpoint envanteri: `rgsx_web.py` + `rgsx_manager.py`'deki tüm `/api/*` handler'ları çıkar.
- Response şekli, hata kodları, SSE event sırası (`snapshot/progress/history/queue/downloaded`)
  için request-level testler (HTTP server'ı mock process ile ayağa kaldırarak veya handler'ı
  doğrudan çağırarak).
- `.coveragerc`'ten kritik dosyaların omit'i tek tek kaldırılarak kapsam gerçek sayılara taşınır
  (öncelik: `qbittorrent_backend.py` → `rgsx_settings.py` → `rgsx_manager.py`).

**Dosyalar:** `tests/` (yeni `test_api_contract.py`, `test_qbittorrent_backend.py`), `.coveragerc`.

**Doğrulama:** 151 test mevcut haliyle geçer + yeni contract testleri kapsama girer; omit
kaldırınca kapsam düşüşü belgelenir.

---

## Faz 8 — Download item state modeli genişletmesi

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

**Doğrulama:** Geçici hata → `retry_count` artar ve tekrar dener; kalıcı hata → `FAILED_PERMANENT`.
`history.json` eski formatla yazılmış veriyi okuyabilir (regression).

---

## Faz 9 — Filtreli listeyi toplu indirme ("Tümünü İndir")

**Amaç:** Filtrelenmiş listenin tek seferde kuyruğa alınması.

**Neden?** ROM koleksiyonu senaryosu; şu an tek oyunla sınırlı (`_process_queued_download`
tek tek işliyor). `_set_bulk_history_status` yalnızca dahili history güncellemesi.

**Kapsam (kararlaştırılan akış):**
- Checkbox modu yok; listenin **ilk satırına** "Tümünü İndir" eklenir.
- Kapsam: **her zaman o an ekranda görünen (filtrelenmiş) set** — ham platform kataloğu değil.
- Zaten indirilmiş oyunların dahil edilmesi kullanıcıya bırakılır; zorunlu atlama yok.
  `game_filters.py`'deki `hide_downloaded` filtresi zaten mevcut — ayrı mekanizma gerekmez.
- Hafif önlem (zorunlu değil): "N/toplam zaten indirilmiş" sayacı veya kuyruk öncesi kısa onay.

**Uygulama:** `/api/download/batch` — mevcut `/api/download` mantığını liste üzerinde döngüye
sokan ince sarmalayıcı; her item mevcut `QUEUED → DOWNLOADING → ...` akışına girer. Yeni state
machine gerekmez. Faz 8'in `FAILED_TRANSIENT`/`RETRY_SCHEDULED` ayrımına dayanır.

**Dosyalar:** `rgsx_manager.py` (endpoint), `rgsx_web.py` (Web UI satırı), `display/game_list.py`
(TV UI satırı), `network.py` (`_process_queued_download` çoklu-destek).

**Doğrulama:** 200 oyunluk filtrelenmiş listede batch kuyruğa girer; `hide_downloaded` açıkken
indirilmişler atlanır; transient hatalar retry'lenir.

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
| `qbittorrent_backend.py` (embedded torrent, `librqbit` adayı) | Windows **+** Linux/Batocera | ⏸ Faz 10b — Linux/ARM test imkânı şart |

**Ara mimari:** Rust manager binary, mevcut `qbittorrent_backend.py`'yi subprocess olarak
çağırmaya devam eder (JSON-RPC veya local HTTP köprüsü). Windows tarafı kademeli Rust'a geçerken
Linux/Batocera Python'da kalır.

**Sözleşme:** `/api/*` ve `/api/events` (SSE) — mevcut davranış birebir korunur; Faz 7'daki
characterization tests bunun garantisidir.

**Stack:** `tokio` + `axum` (HTTP/SSE), `windows-rs` (registry + firewall COM), `serde` (JSON).
Cross-platform genişlerse `cross-rs`/musl toolchain ile ARM cross-compile.

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
Faz 6 (Büyük dosya refaktörü: 6-1..6-5)
   ↓
Faz 7 (Characterization tests)
   ↓
Faz 8 (Download state modeli)   ← Faz 9'un önkoşulu
   ↓
Faz 9 (Toplu indirme)
   ↓
Faz 10 (Rust refaktör)          ← ancak Faz 1-9 sonrası
```

Gerekçeler:
- **1 → 2 → 3**: hızlı, düşük riskli, bağımsız düzeltmeler (P0 + ~10 dk'lık işler).
- **4 (watchdog)**: en yüksek değer — crash durumunda sessiz kayıpları önler.
- **5 (migration)**: P0 ile aynı güvenlik sınıfı; Faz 1'de kurulan şifre sabiti üzerine inşa edilir.
- **7 (tests) → 8 (state) → 9 (bulk)**: state modeli bulk'un, characterization tests Rust'ın önkoşulu.
- **10 (Rust)**: en son — ancak davranış sabitlendikten (Faz 7) ve yeni özellikler oturduktan sonra.
