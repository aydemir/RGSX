# Geliştirici Rehberi — Ortam, İş Akışı ve Konvansiyonlar

> Bu doküman geliştirici ortamı, değişiklik → doğrulama → commit → push döngüsünü
> tanımlar. AGENTS.md'nin genişletilmiş halidir (kod dışı proje kuralları için AGENTS.md).

## 1. Depo ve branch modeli

| Öğe | Değer |
|---|---|
| Depo | `C:\Users\lv\RGSX\RGSX` (çalışma kopyası), git repo kökü |
| Branch | `custom` (geliştirme), `main` (upstream takibi) |
| Remote'lar | `origin` = `RetroGameSets/RGSX` (push reddeder), `aydemir` = `github.com/aydemir/RGSX` (**push hedefi**) |
| Paket | `ports\RGSX` (tek Python paketi, iki mod: TVUI + manager daemon) |
| Test kurulum | `C:\RetroBat - Kopya\roms\ports\RGSX` (doğrulama hedefi) |

**Push kuralı:** `origin`'e push reddedilir → daima `aydemir` remote'una push et
(`git push aydemir custom`).

## 2. Geliştirme ortamı (Windows + Scoop)

Tüm araçlar Scoop ile kurulur (manuel kurulum/msi yok):

```powershell
scoop install git busybox curl wget 7zip cacert python nodejs-lts uv dark
```

| Bileşen | Sürüm | Not |
|---|---|---|
| Python | 3.14.6 (scoop) | `better-sqlite3` nedeniyle `depo-v2` Node v24 ile çalışır |
| Node.js | 24.18.0 (nodejs-lts) | npm global: `pm2` |
| VS Build Tools | 2022 | Native modüller için |
| Process yönetimi | `pm2 resurrect` | Bilgisayar açılışında process listesi geri yüklenir |

### Python bağımlılıkları

```powershell
pip install pygame-ce pytest pytest-cov
```

## 3. Değişiklik döngüsü (zorunlu sıra)

### Adım 1 — Kodu anla

- Değişiklik öncesi **ilgili dosyayı ve çağıranlarını** oku. Büyük modüllerde
  (`controls/` 5.2k, `network/` 6.4k, `utils/` 5.7k — paketler) CodeGraph veya grep ile
  sembol bazlı git.
- `codegraph explore "<sembol adı>"` — bir çağrıda sembol kaynağı + çağıranlar + blast radius.
- Yeni modül bölmesi yaparken AGENTS.md "God Object Decomposition" kurallarını izle.

### Adım 2 — Değişikliği yap

- Kod stili: mevcut deseni takip et (fonksiyon bazlı, `config.*` global state okur).
- Yeni fonksiyonlar için tip hint'leri (`-> ReturnType`).
- **Yorum ekleme** (proje kuralı: gereksiz yorum yok).

### Adım 3 — Test yaz / çalıştır

```bash
pytest --cov=. --cov-report=term-missing
```

- Yeni mantık için `tests/` altına test yaz (bkz. `docs/guides/TESTING.md`).
- Kapsam gate: refaktör edilen modüllerde **%80+**.
- SDL izolasyonu otomatiktir (`conftest.py` dummy driver).

### Adım 4 — Kopya kurulumda canlı doğrula

```powershell
Copy-Item ports\RGSX\*.py "C:\RetroBat - Kopya\roms\ports\RGSX\" -Force
```

1. İlgili modülleri kopyala (değişen dosyalar).
2. Manager'ı başlat ve sağlık doğrula:

```powershell
Start-Process pythonw.exe -ArgumentList "rgsx_manager.py","--minimized" `
  -WorkingDirectory "C:\RetroBat - Kopya\roms\ports\RGSX"
Start-Sleep 6
Get-NetTCPConnection -LocalPort 5000 -State Listen     # port dinliyor olmalı
Invoke-WebRequest http://127.0.0.1:5000/ -UseBasicParsing  # HTTP 200
```

3. Doğrulama sonrası manager'ı durdur: `Stop-Process` (ilgili PID).

### Adım 5 — Commit + push

- Commit mesajı **Türkçe**; tip prefix'i: `feat:`, `fix:`, `refactor:`, `docs:`, `test:`.
- Kapsam: sadece niyet edilen dosyalar (`git add` seçici; secret yok).
- `FEATURES.md` changelog girişi ekle (yeni özellik/fix için).
- Push: `git push aydemir custom`.

```bash
git add <dosyalar>
git commit -m "fix: ..."
git push aydemir custom
```

## 4. Dokümantasyon kuralları

- Belgeler **proje kökündeki `/docs`** klasörüne yazılır. `ports/RGSX/docs` **kullanılmaz**.
- Yapı: `docs/README.md` (indeks) + `architecture/`, `flows/`, `guides/`, `user/`,
  `features/`, `roadmap/`, `deprecated/`.
- Dokümanlar Türkçe; kod terimleri (satır no, fonksiyon adı) İngilizce sembol olarak kalır.
- Dev dokümanı yazarken **kodu önce doğrula** (satır referansları güncel olsun).

## 5. Modül haritası (rol + ölçek)

> Faz 6 refaktörü sonrası (2026-08-11): eski monolitler paket oldu (`utils/`, `network/`,
> `rgsx_web/`, `controls/`); `__main__.py` inceltildi (boot `tvui.py`'ye, spawn/supervisor
> `manager_launcher.py`'ye taşındı). Satırlar `git wc` ile ölçüldü.

| Modül | Rol | Satır |
|---|---|---|
| `display/` (paket, 22 modül) | TVUI ekran/UI çizimi; `OVERLAY` `core.py`'de | 7227 |
| `network/` (paket, 10 modül) | İndirme/torrent mantığı; `queue.py` worker; `download_state.py` Faz 8 state machine | 6454 |
| `utils/` (paket, 13 modül) | Yardımcılar (tar, zip, cache, torrent URL parse, security, extract) | 5672 |
| `controls/` (paket, 6 modül) | Kontrol/input + filtre menüsü + indirme (`downloads.py`) | 5280 |
| `static/js/app.js` | WebUI frontend | 2831 |
| `rgsx_web/` (paket, 9 modül) | Web sunucusu `RGSXHandler` + `/api/download/batch` | 2813 |
| `tvui.py` | TVUI boot + ana döngü (`main()`) | 1862 |
| `qbittorrent_backend.py` | Gömülü qBittorrent backend | 1748 |
| `rgsx_manager.py` | Daemon + tray + SSE; `ManagerHandler` | 1046 |
| `rgsx_cli.py` | CLI komutları | 879 |
| `rgsx_settings.py` | JSON ayar depolama + `get/set_*` accessor'ları | 862 |
| `history.py` | İndirme geçmişi I/O (+ `normalize_downloaded_game_name`) | 783 |
| `config.py` | Ayarlar / `Game` sınıfı / `game_filter_obj` | 730 |
| `language.py` | Çeviri (`_()`) | 424 |
| `game_filters.py` | Filtre modeli + `apply_filters` | 329 |
| `thread_safety.py` | Merkezi kilitler (RLock context manager'lar) | 303 |
| `manager_launcher.py` | Manager spawn/supervisor (watchdog tabanlı) | 262 |
| `__main__.py` | Giriş noktası — yalnız DPI + logging bootstrap + dispatch → `tvui.main` | 111 |
| `watchdog.py` | Hysteresis monitor + restart limiter (saf) | 97 |

## 6. Bilinen önemli davranışlar (hızlı başvuru)

- **Çift manager koruması:** `manager_launcher.ensure_manager()` + `manager_healthy()` — iki manager yarışmaz.
- **Port çakışma:** 5000 doluysa 5000+N; başka process asla öldürülmez.
- **Auto-start:** varsayılan AÇIK (`get/set_autostart_on_boot`); registry `HKCU\...\Run`.
- **WebUI perf:** image cache `public, max-age=3600`; snapshot sonrası skip-render.
- **Filtre bug'ı (b874b99):** `config.game_filter_obj` guard'ı `is None` ile yapılır,
  `hasattr` ile değil.

## 7. Test kurulum servis sağlığı kontrolü

```powershell
Get-NetTCPConnection -LocalPort 5000 -State Listen | Select OwningProcess
Invoke-WebRequest http://127.0.0.1:5000/api/health -UseBasicParsing
```

Health yanıtı `{"success": true, "manager": true, ...}` olmalı.
