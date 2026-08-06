# AGENTS.md — RGSX (RetroBat Game Set Xtreme)

RetroBat TVUI tabanlı oyun indirme/kurma sistemi. Tek Python paketi (`ports\RGSX`), iki çalışma modu: **TVUI** (retro oyun tarayıcı) ve **arka plan daemon** (RGSX Download Manager: tray ikonu + SSE + auto-start).

## Başlatma ve mod ayrımı

- `__main__.py` giriş noktasıdır. `--manager`, `--minimized`, `--web` gibi flag'lere göre TVUI / web / manager moduna girer.
- RetroBat TVUI lansmanı: `roms\windows\RGSX Retrobat.bat` → `__main__.py` → `ensure_manager()` (~satır 679).
- **Çift manager koruması (kritik mimari):** `ensure_manager()` önce `_manager_healthy()` kontrol eder; True ise yeni manager SPAWN ETMEZ. Son savunma: `rgsx_manager.py` main() → `manager_healthy()` True ise "already running" deyip tray ikonu oluşturmadan çıkar.

## Modül haritası (rol + satır ölçeği)

| Dosya | Rol | Satır |
|---|---|---|
| `display.py` | TVUI ekran/UI çizimi (pygame) | 6154 |
| `network.py` | İndirme/torrent mantığı; `InsufficientDiskSpaceError` | 5731 |
| `controls.py` | Kontrol/input işleme | 4514 |
| `utils.py` | Yardımcılar (tar, zip, cache) | 4209 |
| `static/js/app.js` | WebUI frontend | 2734 |
| `rgsx_web.py` | Web sunucusu: `RGSXHandler(BaseHTTPRequestHandler)` | 2221 |
| `__main__.py` | Giriş noktası, mod ayrımı, `ensure_manager` | 1802 |
| `rgsx_cli.py` | CLI komutları (`--auto-start-install/remove` vb.) | 815 |
| `controls_mapper.py` | Input eşleme | 758 |
| `config.py` | Ayarlar/`Game` sınıfı | 634 |
| `rgsx_settings.py` | JSON ayar depolama; `get/set_autostart_on_boot` (dosya sonu ~724+) | 626 |
| `rgsx_manager.py` | Arka plan daemon + tray + SSE; `ManagerHandler(RGSXHandler)` | 577 |
| `static/css/app.css` | WebUI stiller | 532 |
| `history.py` | İndirme geçmişi | 490 |
| `language.py` | Çeviri | 400 |

## Bilinen önemli davranışlar

- **Auto-start:** varsayılan AÇIK. `rgsx_settings.py` → `get/set_autostart_on_boot`; `rgsx_manager.py` `_get_autostart_pref`/`_set_autostart_pref` (~475). Manager ilk başladığında pref True ise registry'ye kurar; tray toggle / CLI tercihi günceller. Registry: `HKCU\...\Run\RGSXManager` → `pythonw.exe rgsx_manager.py --minimized`.
- **Servisler:** manager port 5000; web UI port 4747 (opencode-mem) ile karıştırma. Runtime servis portu `rgsx_manager.py`'den gelir.
- **WebUI perf kararları:** image cache `public, max-age=3600` (rgsx_web.py); app.js session buster + snapshot sonrası skip-render + soft refresh (platform listesi tam re-render edilmez).
- **Windows gamelist.xml:** video/resim görünmeme fix'i uygulandı (`update_gamelist_windows.py`).

## Çalışma kuralları

- Commit mesajları ve `FEATURES.md` changelog **Türkçe**.
- Branch `custom`; push: `origin` (RetroGameSets) reddeder → `aydemir` = `https://github.com/aydemir/RGSX.git` (push fast-forward).
- RetroBat kurulumu test hedefi: `C:\RetroBat - Kopya\roms\ports\RGSX` (manager PID/port doğrulama için `Get-NetTCPConnection -LocalPort 5000`).
- Değişiklik öncesi ilgili dosyayı ve çağıranını okumadan düzenleme yapma; büyük dosyalarda (display/network/controls/utils) CodeGraph/grep ile sembol bazlı git.
- Doğrulama: Kopya kurulumunda çalıştır, servis sağlığı + JSON ayarları teyit et, sonra commit+push.

## Hafıza izolasyonu (opencode-mem)

- Bu repo kendi izole hafıza shard'ına sahiptir (git repo bazlı kimlik). `.opencode-mem-project` marker dosyası **EKLEME** — varlığı birden çok repo/klasörün hafızasını tek havuzda birleştirir (paylaşım = sızıntı).
- Hafıza sorguları **sadece proje scope** kullanılır: `scope: "project"` / varsayılan. `all-projects` scope'u **kullanma** — RGSX özelinde tutulan bilgiyi başka projelere sızdırır.
- Kullanıcı profili (`memory({mode:"profile"})`) bilinçli olarak **cross-project** bilgidir (tercihler, konuşma stili); RGSX mimari kararları oraya değil proje hafızasına yazılır.
- Repo dışı değerlendirme/görevlerde (ör. `C:\Users\lv\RGSX\RGSX` dışında opencode açıldığında) bu hafızaya erişim yoktur — beklendiği gibi izoledir.
