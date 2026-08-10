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

## Faz 3 — qBittorrent WebUI port fallback (Windows)

**Amaç:** 18572 doluysa torrent backend'inin alternatif porta geçmesi (Linux davranışıyla aynı).

**Neden?** Doğrulandı: `_TARGET_PORT=18572` hardcoded, `_ensure_qbittorrent_running()`
içinde `for candidate_port in [_TARGET_PORT]` tek aday (`qbittorrent_backend.py:501`).
Windows'ta çakışma durumunda backend sessizce başarısız olabilir; Linux zaten
`_find_free_webui_port()` kullanıyor.

**Uygulama:** `rgsx_manager.py:821`'deki `_find_available_port()` mantığı `qbittorrent_backend.py`'ye
taşınır (veya ortak helper'a çıkarılır); Windows dalında `webui_port = _find_available_port(_TARGET_PORT)`.
Gerçek port `_base_url`'e zaten dinamik atanıyor — tek değişiklik aday seçimi.

**Dosyalar:** `qbittorrent_backend.py`, muhtemelen `rgsx_manager.py` (helper paylaşımı).

**Doğrulama:** 18572'yi işgal edip backend'in alternatif porta geçtiğini ve indirmenin çalıştığını gösteren canlı test.

---

## Faz 4 — Watchdog / auto-restart

**Amaç:** `/api/health`'i periyodik poll eden, hysteresis'li watchdog thread; process çökünce otomatik restart.

**Neden?** Doğrulandı: `manager_healthy()` tek seferlik kontrol; sürekli poll eden thread yok.
Process çalışırken çökerse indirmeler sessizce takılı kalır.

**Kapsam — iki seviye:**
1. **Manager process:** `INIT → PORT_RESOLVING → FIREWALL_CHECK/CONFIGURING/VERIFIED →
   SUBSYSTEMS_STARTING → RUNNING ⇄ DEGRADED → UNRESPONSIVE → RESTARTING → CRASHED`
2. **qBittorrent backend:** `STOPPED → STARTING → PORT_RESOLVING → WEBUI_AUTH_WAIT →
   RUNNING ⇄ UNRESPONSIVE → RESTARTING`

**Uygulama:**
- `rgsx_manager.py` içinde watchdog thread: her N sn `/api/health` poll; M ardışık FAIL →
  `DEGRADED`, daha uzun → `UNRESPONSIVE`; ardından mevcut `_restart_manager_for_settings()`
  (`rgsx_manager.py:685`) deseniyle spawn-based restart tetiklenir.
- **Hard-crash self-restart imkansız (aynı process öldü).** Çözüm: dış supervisor.
  Tray process'i (mevcut `pystray` tray) supervisor rolü üstlenir — manager'ın çöktüğünü
  fark edip yeniden spawn eder. Tray yoksa (`--no-tray`) Task Scheduler alternatifi belgelenir.
- qBittorrent tarafı: `_ensure_qbittorrent_running()` kendi içinde zaten yeniden başlatabiliyor;
  buna `_wait_for_webui` + `_login` retry döngüsüne sınırlı yeniden deneme ve UNRESPONSIVE
  tespiti eklenir.

**Dosyalar:** `rgsx_manager.py`, `__main__.py` (tray supervisor rolü), `qbittorrent_backend.py`.

**Doğrulama:** Process'i kill edip supervisor'ın yeniden spawn ettiğini gözlemle; qBittorrent'i
öldürüp backend'in RESTARTING→RUNNING geçişini logla.

---

## Faz 5 — qBittorrent şifre migration v1 (DRIFT düzeltmesi)

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

**Doğrulama:** Üç senaryo testi: (a) alan yok → rastgele üretilir; (b) alan = öntanımlı → rastgele
üretilir + flag yazılır; (c) alan = kullanıcı tanımlı → dokunulmaz. İkinci başlatmada flag nedeniyle
hiçbir şey yapılmadığını doğrula.

---

## Faz 6 — Test altyapısı: characterization tests (Rust önkoşulu)

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

## Faz 7 — Download item state modeli genişletmesi

**Amaç:** Mevcut durumlara transient/permanent hata ayrımı ve retry eklemek; sözlük yerine açık model.

**Neden?** Doğrulandı: `Queued/Paused/Connecting/Extracting/Converting/Seeding` zaten var, ama
hata tek "Failed" gibi ele alınıyor; retry yok. Toplu indirmede (Faz 8) rate limit / geçici ağ
kesintisi kaçınılmaz — bu faz Faz 8'in **önkoşulu**.

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

## Faz 8 — Filtreli listeyi toplu indirme ("Tümünü İndir")

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
machine gerekmez. Faz 7'nin `FAILED_TRANSIENT`/`RETRY_SCHEDULED` ayrımına dayanır.

**Dosyalar:** `rgsx_manager.py` (endpoint), `rgsx_web.py` (Web UI satırı), `display/game_list.py`
(TV UI satırı), `network.py` (`_process_queued_download` çoklu-destek).

**Doğrulama:** 200 oyunluk filtrelenmiş listede batch kuyruğa girer; `hide_downloaded` açıkken
indirilmişler atlanır; transient hatalar retry'lenir.

---

## Faz 9 — Rust kısmi refaktör (EN SON)

**Amaç:** State machine + concurrency-ağır manager'ı Rust'a taşımak; Linux/Batocera desteği kırılmadan.

**Motivasyon:** `enum`+`match` ile compiler-enforced state transition'ları; `librqbit`
(`rqbit` motoru) embedded qBittorrent'i ikame edebilir. **Kısıt:** bu faz ancak Faz 1-8
tamamlandıktan sonra — özellikle Faz 6 (characterization tests) olmadan başlanamaz.

**Platform bölünmesi (doğrulanmış kısıt):**
| Bileşen | Platform kapsamı | Rust'a geçiş |
|---|---|---|
| `rgsx_manager.py` (daemon, tray, autostart, port resolve, SSE, watchdog) | Windows-only | ✅ Faz 9a — risk düşük |
| `qbittorrent_backend.py` (embedded torrent, `librqbit` adayı) | Windows **+** Linux/Batocera | ⏸ Faz 9b — Linux/ARM test imkânı şart |

**Ara mimari:** Rust manager binary, mevcut `qbittorrent_backend.py`'yi subprocess olarak
çağırmaya devam eder (JSON-RPC veya local HTTP köprüsü). Windows tarafı kademeli Rust'a geçerken
Linux/Batocera Python'da kalır.

**Sözleşme:** `/api/*` ve `/api/events` (SSE) — mevcut davranış birebir korunur; Faz 6'daki
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
Faz 6 (Characterization tests)
   ↓
Faz 7 (Download state modeli)   ← Faz 8'in önkoşulu
   ↓
Faz 8 (Toplu indirme)
   ↓
Faz 9 (Rust refaktör)           ← ancak Faz 1-8 sonrası
```

Gerekçeler:
- **1 → 2 → 3**: hızlı, düşük riskli, bağımsız düzeltmeler (P0 + ~10 dk'lık işler).
- **4 (watchdog)**: en yüksek değer — crash durumunda sessiz kayıpları önler.
- **5 (migration)**: P0 ile aynı güvenlik sınıfı; Faz 1'de kurulan şifre sabiti üzerine inşa edilir.
- **6 (tests) → 7 (state) → 8 (bulk)**: state modeli bulk'un, characterization tests Rust'ın önkoşulu.
- **9 (Rust)**: en son — ancak davranış sabitlendikten (Faz 6) ve yeni özellikler oturduktan sonra.
