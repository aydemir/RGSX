# RGSX EmulationStation Entegrasyonu - Analiz

> ⚠️ **Durum: DEPRECATED (tarihsel referans).** Bu analiz manager daemon'dan **önceki**
> dönemde (aria2c, port 6999, `rgsx_service.py`) yazılmıştır. O zamandan beri uygulanan
> gerçek mimari farklıdır: `rgsx_manager.py` daemon'ı (port 5000, pystray tray, SSE, gömülü
> qBittorrent) ile tüm indirme delegasyonu çözüldü. Güncel mimari için
> `docs/architecture/DOWNLOAD_MANAGER.md` ve `docs/flows/STARTUP.md`'ye bakın.
> Bu dosya yalnızca karar geçmişini belgelemek için korunur.

## Mevcut Durum

```
RetroBat/EmulationStation
    ↓ (port olarak başlatır)
RGSX (Pygame - bağımsız uygulama)
    ↓
ROM indirme
```

**Sorun:** RGSX bağımsız bir Pygame uygulaması. ES'den ayrı çalışıyor, focus sorunları var, kendi UI'ını çiziyor.

---

## Seçenekler

### Seçenek 1: ES Custom System (Kolay - Orta)

RGSX'i "Retro Game Sets" adında bir ES sistemi olarak ekle.

```
EmulationStation
    ↓ (sistemi seç)
    ↓ (command: python rgsx_launcher.py)
RGSX (Pygame)
```

**Gerekenler:**
- `es_systems_rgsx.cfg` - Sistem tanımı
- `rgsx_launcher.py` - Başlatıcı script
- `themes/rgsx/` - Tema dosyaları
- `roms/ports/` - Boş ROM klasörü

**Avantaj:** Kolay kurulum, mevcut yapıyı bozmaz
**Dezavantaj:** Yine bağımsız uygulama, sadece ES'den başlatılıyor

---

### Seçenek 2: ES Script Integration (Orta)

ES'in script event'lerini kullanarak entegrasyon.

```
EmulationStation
    ↓ (game-start event)
    ↓ (scripts/game-start/rgsx_hook.sh)
RGSX Service (arka plan)
    ↓
ROM indirme devam eder
```

**Gerekenler:**
- `/userdata/system/scripts/rgsx_service` - Service script
- `/userdata/system/configs/emulationstation/scripts/game-start/` - ES hook
- `rgsx_service.py` - Download service (daemon)
- REST API endpoint'leri

**Avantaj:** ES ile tight entegrasyon, background download
**Dezavantaj:** ES'in script sistemine bağımlı

---

### Seçenek 3: ES Plugin/System Modifier (Zor)

ES'in kaynak kodunu değiştirerek native plugin desteği.

```
EmulationStation (modifiye)
    ↓ (native plugin API)
    ↓
RGSX Plugin (C++/Python)
    ↓
Download service
```

**Gerekenler:**
- ES fork (batocera-emulationstation)
- Plugin API geliştirme
- C++ veya Python binding'leri

**Avantaj:** En temiz çözüm, native performans
**Dezavantaj:** Çok iş, bakım yükü, ES güncellemeleriyle uyumsuzluk

---

### Seçenek 4: Web-based Integration (Önerilen)

RGSX'i tamamen web tabanlı yap, ES'den tarayıcı olarak aç.

```
EmulationStation
    ↓ (command: chromium http://localhost:5000)
    ↓
Web UI (Flask)
    ↓
Download Service (daemon)
```

**Gerekenler:**
- `rgsx_service.py` - Download service (zaten var)
- `rgsx_web.py` - Web UI (zaten var, iyileştirilecek)
- ES'den URL açılması

**Avantaj:**
- Mevcut web UI kullanımı
- Background download (service zaten ayrı process)
- Cross-platform
- Bakım kolaylığı

**Dezavantaj:**
- Pygame UI iptal
- Tarayıcı bağımlılığı

---

## Karşılaştırma Tablosu

| Kriter | Seçenek 1 | Seçenek 2 | Seçenek 3 | Seçenek 4 |
|--------|-----------|-----------|-----------|-----------|
| Zorluk | Kolay | Orta | Zor | Orta |
| ES Entegrasyonu | Düşük | Orta | Yüksek | Yüksek |
| Background Download | Hayır | Evet | Evet | Evet |
| Bakım Yükü | Düşük | Orta | Yüksek | Düşük |
| Cross-Platform | Evet | Hayır | Hayır | Evet |
| UI Kalitesi | İyi | İyi | Mükemmel | İyi |
| Gelecek | Sınırlı | İyi | En iyi | İyi |

---

## Önerilen Yaklaşım: Seçenek 2 + 4 Hibrit

### Faz 1: Download Service (Hemen)
```python
# rgsx_service.py
- Bağımsız daemon process
- REST API (port 6999)
- aria2c yönetimi
- Kuyruk sistemi
```

### Faz 2: Web UI İyileştirme (1-2 hafta)
```python
# rgsx_web.py
- Tamponsuz tam ekran modu
- Controller navigasyonu
- Oyun listesi görünümü
- İndirme yönetimi
```

### Faz 3: ES Entegrasyonu (1 hafta)
```bash
# es_systems_rgsx.cfg
<system>
  <name>rgsx</name>
  <fullname>Retro Game Sets Xtra</fullname>
  <path>/userdata/roms/ports</path>
  <extension>.sh</extension>
  <command>python /path/to/rgsx_launcher.py %ROM%</command>
  <theme>rgsx</theme>
</system>
```

```python
# rgsx_launcher.py
#!/usr/bin/env python
import subprocess
import sys

# Web UI'ı başlat (eğer çalışmıyorsa)
# Tarayıcıyı aç
subprocess.Popen(["chromium", "--app=http://localhost:5000"])
```

### Faz 4: ES Script Hooks (Opsiyonel)
```bash
# /userdata/system/scripts/rgsx_service
#!/bin/bash
case $1 in
  start)
    python /path/to/rgsx_service.py &
    ;;
  stop)
    kill $(cat /var/run/rgsx_service.pid)
    ;;
esac
```

---

## Mimari Diyagram (Önerilen)

```
┌─────────────────────────────────────────────────────────┐
│                    EmulationStation                      │
│  ┌─────────────────────────────────────────────────┐    │
│  │  Retro Game Sets Xtra (Custom System)            │    │
│  │  - Tema: rgsx theme                              │    │
│  │  - Command: rgsx_launcher.py                     │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘
                          │
                          ↓
┌─────────────────────────────────────────────────────────┐
│              rgsx_launcher.py (Basit)                   │
│  - Service çalışıyor mu? kontrol et                    │
│  - Çalışmıyorsa başlat                                  │
│  - Tarayıcıyı/WebView'ı aç                             │
└─────────────────────────────────────────────────────────┘
                          │
          ┌───────────────┴───────────────┐
          ↓                               ↓
┌─────────────────────┐    ┌─────────────────────────────┐
│   Web UI (Flask)    │    │   Download Service (Daemon)  │
│   port: 5000        │    │   port: 6999                 │
│                     │    │                              │
│ - Oyun listesi      │    │ - aria2c yönetimi            │
│ - Arama/Filtre      │←──→│ - Kuyruk sistemi             │
│ - İndirme başlat    │    │ - Durum takibi               │
│ - Durum göster      │    │ - History                    │
└─────────────────────┘    └─────────────────────────────┘
          │                               │
          │      HTTP/JSON API            │
          └───────────────────────────────┘
                          │
                          ↓
┌─────────────────────────────────────────────────────────┐
│              ROM Klasörü (/userdata/roms/)               │
│  - PS2/                                                  │
│  - PSP/                                                  │
│  - N64/                                                  │
│  - ...                                                   │
└─────────────────────────────────────────────────────────┘
```

---

## Gerekenler (Checklist)

### Temel
- [ ] `rgsx_service.py` - Download service daemon
- [ ] `rgsx_service_client.py` - API client
- [ ] `rgsx_launcher.py` - ES başlatıcı
- [ ] `es_systems_rgsx.cfg` - ES sistem tanımı
- [ ] `themes/rgsx/` - Tema dosyaları

### Servis
- [ ] REST API endpoints
- [ ] aria2c entegrasyonu
- [ ] Kuyruk yönetimi
- [ ] Durum takibi (JSON/SQLite)
- [ ] Otomatik başlatma (batocera-services)

### Web UI
- [ ] Controller navigasyonu
- [ ] Tam ekran modu
- [ ] Oyun listesi görünümü
- [ ] İndirme yönetimi
- [ ] Ayarlar sayfası

### ES Entegrasyonu
- [ ] Tema dosyaları
- [ ] Script hooks
- [ ] Service başlatma/durdurma
- [ ] Bildirim desteği

---

## Sonuç

**En pragmatik yaklaşım:** Seçenek 2 + 4 hibrit

1. Download service'ı bağımsız yap (zaten yapılması gereken)
2. Web UI'ı ES'den açılacak şekilde iyileştir
3. ES'e custom system olarak ekle
4. Background download desteği ekle

Bu yaklaşım:
- Mevcut kodu korur
- Background download sağlar
- ES ile entegre çalışır
- Bakım kolaylığı sağlar
- Cross-platform çalışır
