# RGSX Download Manager Mimarisi

## Genel Bakış

RGSX Download Manager, TV UI (Pygame) ve indirme motorunu ayıran bağımsız bir **daemon** process'tir. TV UI kapatılsa bile indirmeler arka planda (sistem tepsisi / tray) devam eder.

## Mimari Bileşenler

```
┌─────────────────────────────────────────────────────────────┐
│                    TV UI (Pygame)                            │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │ display.py  │  │ controls.py │  │ __main__.py         │  │
│  │ - Oyun listesi    │ - Kontrol      │ - ensure_manager()│  │
│  │ - Durum göstergeleri│ - Filtreler   │ - SSE listener    │  │
│  └──────┬──────┘  └──────┬──────┘  └────────┬────────────┘  │
└─────────┼────────────────┼───────────────────┼───────────────┘
          │                │                   │
          ▼                ▼                   ▼
┌─────────────────────────────────────────────────────────────┐
│              RGSX Download Manager (Daemon)                 │
│  ┌─────────────────┐ ┌─────────────────┐ ┌───────────────┐  │
│  │ rgsx_manager.py │ │ rgsx_web.py     │ │ network.py    │  │
│  │ - HTTP + SSE    │ │ - Web UI        │ │ - İndirme     │  │
│  │ - Queue Worker  │ │ - REST API      │ │ - Torrent     │  │
│  │ - Tray Icon     │ │ - Static Files  │ │ - Resume      │  │
│  │ - Auto-start    │ │ - SSE Events    │ │ - 1Fichier    │  │
│  └─────────────────┘ └─────────────────┘ └───────────────┘  │
└─────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────┐
│                    ROM Klasörleri                            │
│  /userdata/roms/PS2/  /userdata/roms/PSP/  ...             │
└─────────────────────────────────────────────────────────────┘
```

## Process Akışı

### 1. Başlatma (`__main__.py` → `ensure_manager()`)

```python
def ensure_manager():
    # 1. Manager sağlıklı mı kontrol et (_manager_healthy)
    # 2. Sağlıksızsa: subprocess.Popen ile rgsx_manager.py başlat
    # 3. Port/host ayarlarını rgsx_settings.json'dan oku
    # 4. SSE listener thread başlat (_start_manager_sse_listener)
    # 4. TV UI normal başlar
```

### 2. Manager Daemon (`rgsx_manager.py`)

```python
class RGSXManager:
    def main():
        # 1. Port kontrolü (_find_available_port)
        # 2. HTTP + SSE sunucusu başlat (rgsx_web.py)
        # 3. Kuyruk işçi thread başlat (download_queue_worker)
        # 4. Tray ikonu oluştur (pystray)
        # 4. Auto-start registry kontrolü
        # 5. Ana döngü: sağlık kontrolü + kuyruk işleme
```

### 3. SSE Event Akışı (Manager → TV UI)

```
Manager                          TV UI (SSE Listener)
    │                                 │
    ├── snapshot ────────────────────►│  config.downloaded_games güncelle
    │                                 │  config.download_queue güncelle
    ├── progress ────────────────────►│  config.download_progress güncelle
    │                                 │  config.needs_redraw = True
    ├── history ─────────────────────►│  config.history güncelle
    ├── queue ───────────────────────►│  config.download_queue güncelle
    ├── downloaded ──────────────────►│  config.downloaded_games + needs_redraw
```

### 4. İndirme İşleme (`network.py`)

```
download_rom() 
    │
    ├── HTTP (1Fichier/AllDebrid/DebridLink/RealDebrid/TorBox)
    │     └── Range/Resume desteği (.part dosyası + Content-Range)
    │
    ├── Torrent (qBittorrent-nox embedded)
    │     └── qbittorrent_backend.py API
    │
    └── Local fallback (RGSX_NO_MANAGER=1 veya --ui-only)
          └── Yerel threading.Thread ile doğrudan
```

## API Endpoints (Manager)

| Endpoint | Metot | Açıklama |
|----------|-------|----------|
| `/api/health` | GET | Manager durumu (`manager`, `version`, `pid`) |
| `/api/events` | GET (SSE) | `snapshot`/`progress`/`history`/`queue`/`downloaded` |
| `/api/download` | POST | İndirme ekle (`game_index`, `game_name`, `url`, `platform`) |
| `/api/cancel` | POST | İndirme iptal (`task_id`) |
| `/api/shutdown` | POST | Manager kapat |
| `/api/pause` | POST | Tüm indirmeleri duraklat |
| `/api/resume` | POST | Tüm indirmeleri sürdür |
| `/api/qbittorrent/password-status` | GET | Şifre durumu (`using_default`) |
| `/api/qbittorrent/change-password` | POST | Şifre değiştir |

## Konfigürasyon

### Manager Ayarları (`rgsx_settings.json`)

```json
{
  "manager_port": 5000,
  "manager_host": "0.0.0.0",
  "autostart_on_boot": true,
  "game_filters": {
    "region_filters": {},
    "hide_non_release": false,
    "one_rom_per_game": false,
    "hide_downloaded": false,
    "regex_mode": false,
    "region_priority": ["USA", "Canada", "World", "Europe", "Japan", "Other"]
  },
  "qbittorrent_webui_password": "custom_password"
}
```

### CLI Seçenekleri

```bash
python rgsx_manager.py [options]
  --port=N              # Port (default: settings'ten)
  --host=HOST           # Host (default: 0.0.0.0)
  --no-tray             # Tray ikonu olmadan
  --minimized           # Minimize başlat
  --auto-start-install  # Registry'ye auto-start ekle
  --auto-start-remove   # Auto-start kaldır
  --no-web              # Web UI olmadan
```

## Cross-Platform Desteğ

| Platform | Tray | Auto-start | Port Serbest Bırakma |
|----------|------|------------|---------------------|
| Windows  | ✅ pystray | ✅ Registry Run | `netstat`/`taskkill` |
| Linux    | ❌ (devre dışı) | ❌ | `lsof` + `kill` |
| Batocera | ❌ | ✅ batocera-services | `lsof` + `kill` |

*Windows-specific import'lar (`winreg`, `pystray`) fonksiyon içinde yapılır, Linux'ta zarifçe devre dışı kalır.*

## Doğrulama ve Testler

- `py_compile` tüm Python dosyaları geçer
- `RGSX_NO_MANAGER=1` / `--ui-only` → Yerel fallback test
- Port çakışma: 5000 dolu → 5001'e geçiş, işgalci süreç öldürülmez
- Auto-start: `--auto-start-install` → Registry yazılır, `--auto-start-remove` → Silinir
- Tray: 5 menü öğesi, Web UI açma, klasörler, auto-start toggle, Exit
- İptal: Yavaş sunucudan 2 indirme → birini iptal → diğer kuyrukta devam
- Linux/Batocera: Cross-platform doğrulandı

---

*Kaynak kod: `rgsx_manager.py`, `rgsx_web.py`, `network.py`, `__main__.py`, `controls.py`, `display.py`*