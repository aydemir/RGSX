# RGSX — Download Manager Pattern Refactor Planı

**Tarih:** 2026-08-04
**Hedef:** TV UI ve Web UI'ı birbirinden bağımsız, indirmeleri senkronize,
system tray'de yaşayan tek bir "manager daemon" etrafında birleştirmek.
Cross-platform (Windows + Linux/Batocera/Knulli).
**Kapsam:** Mimari refactor + yeni dosyalar + mevcut dosyalarda taşıma/silme.
**Kod bu planda YOK** — sadece dosya/satır referansları ve yapılacak iş.

---

## 1. Sorun (bugünkü durum)

`__main__.py` ana süreçte iki iş aynı anda:
- TV UI (pygame mainloop)
- indirme motoru (download_queue_worker thread)

`rgsx_web.py` ise **subprocess olarak** başlatılıyor
(`__main__.py:468-498 start_web_server`, `subprocess.Popen([exe, rgsx_web.py], ...)`).
Sonuç:

- İki ayrı Python süreci → iki ayrı `config` modülü (RAM'de), state paylaşımı
  sadece disk üzerinden (`history.json`, `downloaded_games.json`).
- TV UI'dan başlatılan indirme → Web UI eski listeyi gösterir (polling yok).
- Web UI'dan başlatılan indirme → TV UI görmez.
- TV UI kapanınca Web UI süreci bazen ölüyor (handle/parent-child bağımlılığı).

Yani bugün "iki UI" değil, aslında "iki ayrı uygulama aynı ROM'ları indiriyor
gibi davranıyor, arada dosya tabanlı gevşek senkronizasyon var".

---

## 2. Hedef (download manager pattern)

Tek daemon süreci = **manager**. Tüm state burada. UI'lar sadece render.
Transmission / qBittorrent / Synology Download Station modeli.

```
┌──────────────────────────────────────────────────┐
│  rgsx_manager.py  (tek uzun-ömürlü daemon süreç) │
│  ├─ indirme motoru (download_queue_worker)       │
│  ├─ state: history + downloaded_games + queue   │
│  ├─ REST API   (port 5000)                       │
│  ├─ SSE stream (port 5000, /api/events)          │
│  ├─ system tray (pystray)                        │
│  │   sağ-tık menü: TV UI · Web UI · Durum · Çıkış│
│  └─ auto-start (Windows registry + Linux .desktop)│
└──────────────────────────────────────────────────┘
       ▲                              ▲
       │ HTTP + SSE                   │ HTTP + SSE
       │                              │
┌──────┴─────────┐            ┌───────┴────────┐
│  TV UI client  │            │  Web UI        │
│  (pygame)      │            │  (browser)     │
│  sadece render │            │  sadece render │
└────────────────┘            └────────────────┘
```

**SSE seçimi** (WebSocket yerine):
- Browser native `EventSource` — Web UI'da sıfır kod
- TV UI için basit HTTP streaming read (5-10 satır `requests`)
- Reverse proxy arkasında sorunsuz (ileride cloud)
- Daha az bağlantı/state yönetimi

**Cross-platform:**
- `pystray` cross-platform ama Linux'ta AppIndicator extension gerekli
  (Batocera/Knulli GNOME yerine lightweight WM kullanıyor — fallback olarak
  bildirim + CLI komutu yeterli olabilir, ayrıca değerlendirilecek)
- Windows: registry auto-start
- Linux: `~/.config/autostart/rgsx.desktop`

---

## 3. Mevcut yapı envanteri (planlanan değişiklikler)

| Dosya | Bugün | Refactor sonrası |
|-------|-------|------------------|
| `__main__.py` (102 KB, 1873 satır) | TV UI + indirme motoru + subprocess launcher | Sadece TV UI client; açılışta manager'a HTTP ping, çalışmıyorsa uyarı ver veya başlat |
| `rgsx_web.py` (100 KB, 2144 satır) | HTTP server (port 5000), kendi `config` modülü | **İkiye bölünür:** API kısmı manager'a, statik HTML/JS kısmı `static/` klasörüne |
| `network.py` (292 KB, 5758 satır) | İndirme motoru, indirme thread'leri, queue worker | **Yerinde kalır.** Manager tarafından import edilir |
| `display.py` (308 KB, 6542 satır) | TV UI (pygame) | **Yerinde kalır**, ama `config`'e doğrudan erişen yerlerde küçük değişiklik gerekebilir (manager'dan state çekmek için HTTP/SSE) |
| `history.py` (18 KB, 501 satır) | history.json + downloaded_games.json yönetimi | **Yerinde kalır**, manager tarafından çağrılır |
| `rgsx_settings.py` (25 KB, 698 satır) | ayarlar | **Yerinde kalır** |
| `config.py` (32 KB, 701 satır) | global config (RAM'de) | **Yerinde kalır**, sadece manager sürecinde yaşar; TV UI artık kendi kopyasını taşımaz, SSE/HTTP ile çeker |
| `RGSX Retrobat.bat` (Windows launcher) | Tek kısayol, hepsini başlatır | İki kısayol: "RGSX Manager" (auto-start), "RGSX TV UI" (manager çalışıyorken UI açar) |

### Yeni dosyalar

| Dosya | Sorumluluk |
|-------|------------|
| `rgsx_manager.py` | Daemon: queue worker + REST + SSE + tray + auto-start |
| `rgsx_manager_service.py` (veya fonksiyon) | Manager'ın Windows service / systemd unit olarak çalışması için ince wrapper (opsiyonel, ilk aşamada gerekli değil) |
| `rgsx_tv_client.py` (veya `__main__.py`'nin kendisi) | TV UI sadece render katmanı; manager'a SSE ile bağlanır |
| `static/` klasörü | Web UI HTML/JS/CSS (mevcut `rgsx_web.py`'den çıkar) |

---

## 4. Refactor adımları (sıra önemli)

### Adım 1 — Manager iskeletini oluştur (yeni dosya)
- `rgsx_manager.py` yaz (kodu bu planda yok, sadece sorumluluğu):
  - mevcut `download_queue_worker`'ı import et ve çalıştır
  - mevcut `rgsx_web.py`'nin HTTP route'larını içine al
  - SSE endpoint ekle: `/api/events` → state değişimlerini push'la
  - `pystray` ile tray ikonu başlat
  - args: `--no-tray`, `--port=5000`, `--auto-start-install`, `--auto-start-remove`
- Tek dosyada 500-800 satır olur, yönetilebilir.

### Adım 2 — `rgsx_web.py`'yi parçala
- Mevcut HTTP handler'lar → `rgsx_manager.py`'ye taşı (veya import et)
- Statik HTML/JS/CSS → `static/` altına taşı (zaten `static/` klasörü var,
  `__main__.py:1033` /api/image/ gibi endpointler orada)
- `rgsx_web.py` ya tamamen silinir ya da ince uyumluluk shim'i olarak kalır

### Adım 3 — `__main__.py`'yi sadeleştir
- TV UI açılışında `http://127.0.0.1:5000/api/health` ping at
  - 200 OK → manager çalışıyor, bağlan, SSE aç
  - başarısız → kullanıcıya "Manager çalışmıyor, başlatmak ister misin?" diyaloğu
    (veya `--auto-start` arg ile sessizce başlat)
- TV UI state'i artık doğrudan `config`'ten değil, **manager'dan HTTP/SSE ile** gelir
- pygame mainloop aynı kalır, sadece state kaynağı değişir
- `start_web_server()` (satır 468-498) **silinir** — subprocess'a gerek yok

### Adım 4 — SSE event formatı
Manager iç event'leri:
```
event: queue
data: {"url": "...", "platform": "snes", "game": "Game Name", "status": "Downloading", "progress": 42}

event: history
data: {"platform": "snes", "game": "Game Name", "status": "Download_OK", "timestamp": "..."}

event: downloaded
data: {"platform": "snes", "game": "game normalized name"}

event: settings
data: {"max_simultaneous_downloads": 5, ...}
```
- TV UI bu stream'i `requests` ile okur, pygame event queue'ya inject eder
- Web UI `EventSource` ile direkt bağlanır
- Mevcut `progress_queues` (network.py:3058) mekanizması manager içinde kalır,
  event'ler oradan üretilir

### Adım 5 — System tray (pystray)
- Tray ikonu: PNG, 16x16 ve 32x32 (mevcut `static/` veya `assets/`'dan)
- Sol tık: küçük popup — aktif indirme sayısı, tamamlanan
- Sağ tık menü:
  - **TV UI aç** → `python __main__.py` (subprocess veya HTTP link)
  - **Web UI aç** → varsayılan tarayıcıda `http://127.0.0.1:5000`
  - **Durum** → indirme listesi özet
  - **Çıkış** → manager'ı temiz kapat
- Linux'ta AppIndicator yoksa: tray menüsü `zenity`/`notify-send` ile basit
  bildirim + "Web UI'ı açmak için tarayıcıdan girin" mesajı (fallback)

### Adım 6 — Auto-start
- Windows: `winreg.HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`
  - anahtar: `RGSX Manager`
  - değer: `python rgsx_manager.py --minimized`
  - `--auto-start-install` / `--auto-start-remove` argümanları
- Linux: `~/.config/autostart/rgsx-manager.desktop` yaz
  - `Exec=python /path/to/rgsx_manager.py`
  - `Type=Application`
  - `X-GNOME-Autostart-enabled=true`

### Adım 7 — Batch dosyasını güncelle
`RGSX Retrobat.bat`:
- Yeni argümanlar:
  - `--manager` → manager'ı başlat (varsayılan: manager yoksa başlat, varsa sadece bağlan)
  - `--ui` → TV UI aç (manager zaten çalışıyor olmalı)
  - `--ui-only` → eski davranış: TV UI + manager birlikte (geriye uyumluluk)
- Yeni kısayollar:
  - **RGSX Manager** → sadece manager + tray
  - **RGSX TV UI** → manager çalışıyorsa sadece TV UI
  - **RGSX Web UI** → manager çalışıyorsa tarayıcıyı aç
  - **RGSX (eski)** → mevcut davranış, geriye uyumluluk

### Adım 8 — State sahipliği temizliği
- `config.py`'deki global değişkenler (`config.download_queue`,
  `config.download_progress`, `config.download_tasks` vb.) sadece
  **manager sürecinde** yaşar
- TV UI süreci bu değişkenleri taşımaz; manager'dan çeker
- `history.json` + `downloaded_games.json` disk formatı aynı kalır
  (geriye uyumluluk)

---

## 5. Davranış değişiklikleri (kullanıcı gözünden)

| Senaryo | Bugün | Refactor sonrası |
|---------|-------|------------------|
| `RGSX Retrobat.bat` çalıştır | TV UI + Web UI başlar (subprocess) | Sadece TV UI başlar; manager zaten çalışıyorsa bağlanır, değilse başlatır |
| TV UI'dan indirme başlat | Web UI refresh yapmazsa görmez | Manager SSE push'lar, Web UI anında görür |
| Web UI'dan indirme başlat | TV UI görmez | Aynı SSE, TV UI da anında görür |
| TV UI kapat | Web UI da bazen ölür | Manager çalışmaya devam eder, Web UI açık kalır |
| Web UI kapat (tarayıcı sekmesi) | TV UI etkilenmez | TV UI etkilenmez, manager çalışır |
| Bilgisayarı kapat | Manager kapanır, indirmeler yarım | Auto-start + manager: bilgisayar açılınca manager başlar, kuyrukta bekleyen indirmeler devam eder (mevcut `reprendre les téléchargements interrompus` mantığı korunur) |
| `Alt+Tab` ile manager'a dön | Pencere yok, gizli | Tray ikonuna sol tık → popup, sağ tık → menü |

---

## 6. Kapsam dışı (bu planda YOK)

- AriaNg veya başka bir dış UI entegrasyonu (gerek yok, kendi tray + Web UI yeterli)
- Bulut senkronizasyonu
- Mobil uygulama (Web UI mobil uyumlu, yeterli)
- RGSX'in indirme motorunu değiştirme (network.py yerinde kalıyor)
- Rom filter UI fikri (önceki oturumda not alındı, ayrı iş)

---

## 7. Karar verilen noktalar

1. **Eski `rgsx_web.py` → shim olarak kalsın.**
   - Shim davranışı: manager çalışıyorsa istekleri ona yönlendir (proxy/reverse),
     manager çalışmıyorsa eski davranışı sürdürür (TV UI olmadan da çalışabilsin).
   - Geriye uyumluluk değerli; eski kısayollar kırılmasın.

2. **Linux'ta tray fallback:** AppIndicator varsa tray, yoksa `notify-send` + log
   fallback. Tray'i hiç başlatmamak kötü UX.

3. **Tek port (5000).** Path-based ayrım: `/api/*` REST, `/api/events` SSE.

4. **Manager auto-start default.** B + C hibrit:
   - Auto-start ile sistem açılışında manager başlar
   - TV UI ilk açılışta "manager yok" diyaloğu gösterebilir (fallback)
   - Geriye uyumluluk: `RGSX Retrobat.bat` eski davranış (hepsini başlat) korunur

5. **`config.py` global değişkenler → class-based proxy.**
   - TV UI'da `config` modülü `ManagerConfigProxy` instance'ı döner
   - `config.download_queue` gibi erişimler proxy'nin `__getattr__` üzerinden
     cache'lenmiş snapshot'tan okunur (5-10s periyodik refresh)
   - Hot-path event'leri SSE üzerinden local state'e yazılır, böylece her
     frame'de HTTP roundtrip yapılmaz
   - Bu trade-off: küçük staleness (5-10s) kabul edilebilir, hot-path temiz

6. **Tray ikonu:** mevcut `rgsx.png` kullanılır (zaten `static/` veya `assets/`
   altında). Yeni ikon gerekmez.

---

## 8. Riskler ve mitigasyon

| Risk | Etki | Mitigasyon |
|------|------|------------|
| SSE bağlantısı koparsa UI stale olur | Yüksek | Periyodik full state snapshot (her 30s), client reconnect logic |
| Manager çökerse tüm indirmeler ölür | Yüksek | Mevcut `download_queue_worker` zaten thread-safe; ayrıca supervisor (systemd / NSSM) ile restart |
| `pystray` Linux'ta çalışmazsa | Düşük | Fallback: `notify-send` + CLI kontrolü |
| Eski kullanıcılar `--webui` arg ile direkt web açınca ne olur? | Orta | `rgsx_web.py` shim'i manager'a yönlendirir |
| `history.json` aynı anda iki süreçten yazılırsa (geçiş döneminde) | Orta | `_atomic_write_json` (mevcut, history.py:130) zaten var; manager kilitli, TV UI sadece okur |
| TV UI tamamen HTTP'ye bağımlı olunca offline çalışmaz | Düşük | Manager zaten local'de, HTTP local; "offline" zaten başlamış demek |

---

## 9. Tahmini iş (kod yazmıyorum, sadece tahmin)

| Adım | Büyüklük | Not |
|------|----------|-----|
| 1. Manager iskeleti | 1-2 gün | Yeni dosya, temiz mimari |
| 2. rgsx_web.py parçala | 0.5-1 gün | Taşıma + import düzeltme |
| 3. __main__.py sadeleştir | 1-2 gün | State akışı değişiyor, dikkatli ol |
| 4. SSE event formatı + client | 1 gün | İki tarafta da (TV + Web) basit |
| 5. System tray | 0.5-1 gün | pystray basit, ikon lazım |
| 6. Auto-start | 0.5 gün | Platform-specific wrapper'lar |
| 7. Batch güncelle | 0.5 gün | Yeni kısayollar |
| 8. Test + bugfix | 2-3 gün | En önemli kısım |
| **Toplam** | **~7-10 gün** | Tam zamanlı çalışmayla |

---

## 10. Referans satırlar (mevcut kodda kritik noktalar)

- `__main__.py:550` — `async def main()` (giriş noktası, parçalanacak)
- `__main__.py:468-498` — `start_web_server()` (silinecek)
- `__main__.py:575-576` — `download_queue_worker` thread (manager'a taşınacak)
- `network.py:3344` — `async def download_rom()` (yerinde kalır)
- `network.py:4399` — `async def download_from_1fichier()` (yerinde kalır)
- `network.py:3058` — `progress_queues` (manager'a taşınır)
- `network.py:4255-4266` — final message → `mark_game_as_downloaded` (yerinde kalır)
- `rgsx_web.py:1131` — `/api/download` endpoint (manager'a taşınır)
- `rgsx_web.py:1067-1113` — `_process_queued_download` (manager'a taşınır)
- `history.py:425-430` — `mark_game_as_downloaded` (yerinde kalır)
- `history.py:442-446` — `is_game_downloaded` (yerinde kalır)
- `history.py:293-330` — `scan_roms_for_downloaded_games` (manager start-up'ında çağrılır)
- `display.py:2138-2149` — yeşil tik render (`is_game_downloaded` çağrısı, TV UI'da kalır)
- `config.py` — global config (manager-only, TV UI proxy kullanır)

---

## 11. Doğrulama (kabul kriterleri)

1. Manager daemon başlat → tray ikonu gözükür
2. TV UI aç → manager'a bağlanır, indirme listesi sync olur
3. Web UI aç (tarayıcı) → aynı indirme listesi, gerçek zamanlı
4. TV UI'dan indirme başlat → Web UI 1 saniye içinde görür (SSE)
5. Web UI'dan indirme başlat → TV UI 1 saniye içinde görür
6. TV UI kapat → manager ve Web UI çalışmaya devam eder
7. Manager'ı tray'den kapat → tüm UI'lar kapanır, indirmeler yarım kalır
8. Bilgisayarı yeniden başlat → auto-start ile manager gelir, kuyruk devam eder
9. Rom filter UI fikri (önceki oturum) bu plandan bağımsız, ayrı iş
10. Windows + Linux'ta aynı davranış

---

## 12. Web UI frontend stratejisi

Bugünkü Web UI server-side render: tıklayınca tam sayfa yenileme (`/platform/X` → sayfa reload).
Sadece SSE eklemek indirmeleri canlı yapar ama "platform list'ten oyun listesine
geçince scroll/filtre state kaybı, his olarak pürüzlü" sorununu çözmez.

İki aşamalı yol:

### Aşama 1 — SSR + SSE patch (hızlı kazanç, ~1 gün)
- Mevcut template'ler korunur
- Her sayfanın altına EventSource inject edilir
- Event gelince `fetch()` ile güncel state çekilir, **DOM parça parça** günceller
- Navigasyon: tam sayfa yenileme devam eder
- Sorun: scroll state, filtre, sıralama her sayfa değişiminde kaybolur
- Avantaj: küçük risk, geriye uyumlu, **Aşama 2 için önkoşul değil**

### Aşama 2 — SPA / htmx (pürüzsüz UX, ~3-4 gün)
- Tüm Web UI **tek `index.html` + JS** haline gelir
- URL routing: `history.pushState()` ile sanal routing (sayfa yenilenmez)
- SSE EventSource bağlı kalır, event → component re-render
- **Scroll, filtre, sıralama, tema state'i korunur**
- AriaNg / qBittorrent Web UI gibi "native app hissi"

**Framework seçenekleri (Aşama 2):**

| Yol | Büyüklük | Avantaj | Dezavantaj |
|-----|----------|---------|------------|
| Vanilla JS + Web Components | orta | Framework yok, küçük bundle | Çok kod yazılır |
| **htmx + SSE** | küçük-orta | Mevcut template'ler korunur, declarative, ~14 KB, build tooling yok | htmx-öğrenme |
| Alpine.js | küçük | Tiny, reaktif, SSR-uyumlu | Component yapısı zayıf |
| Vue 3 (no-build) | orta | Modern, kolay component | Bundle nispeten büyük |
| Svelte (no-build) | orta | Compile-time, küçük çıktı | Svelte 5 karmaşıklığı |

**Karar (kullanıcı onayı ile):** htmx + SSE.

Aşama sırası (kesinleşti):
1. Önce Aşama 1'i yap (SSE + temel canlı state) — kullanıcı geri bildirimi al
2. Kullanıcı "pürüzsüz navigasyon" isterse Aşama 2'ye geç
3. Aşama 2 framework'ü: **htmx + SSE** (kararlaştırıldı)

---

**Bu plan tamamlandığında:** RGSX "iki ayrı uygulama gibi davranan tek uygulama"
olmaktan çıkıp, "tek daemon + iki ince UI" olan gerçek bir download manager'a
dönüşür. AriaNg veya başka bir dış UI'a gerek kalmaz.
