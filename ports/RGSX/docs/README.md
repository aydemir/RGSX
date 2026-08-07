# RGSX Documentation

Bu klasör RGSX projesinin tüm teknik dokümantasyonunu içerir.

## 📁 Klasör Yapısı

```
docs/
├── README.md                 # Bu dosya - dokümantasyon indeksi
├── features/
│   └── FEATURES.md           # Özellikler ve değişiklik günlüğü (changelog)
├── architecture/
│   ├── ES_INTEGRATION_ANALYSIS.md  # EmulationStation entegrasyon analizi
│   └── DOWNLOAD_MANAGER.md         # RGSX Download Manager mimarisi
├── roadmap/
│   └── ROADMAP.md            # Geliştirme yol haritası
├── guides/
│   ├── TVUI_FILTERS.md       # TVUI filtre kullanım kılavuzu
│   └── WEBUI_FILTERS.md      # WebUI filtre kullanım kılavuzu
└── deprecated/
    └── FOCUS_FIX.md          # Kaldırılan odak düzeltme dökümanı (referans)
```

## 🚀 Hızlı Başlangıç

| Doküman | Açıklama |
|---------|----------|
| [FEATURES.md](features/FEATURES.md) | Tüm özellikler ve sürüm notları |
| [ROADMAP.md](roadmap/ROADMAP.md) | Gelecek planlar ve fazlar |
| [ES_INTEGRATION_ANALYSIS.md](architecture/ES_INTEGRATION_ANALYSIS.md) | ES entegrasyon analizi |
| [DOWNLOAD_MANAGER.md](architecture/DOWNLOAD_MANAGER.md) | Download Manager mimarisi |
| [TVUI_FILTERS.md](guides/TVUI_FILTERS.md) | TVUI filtre kullanımı |
| [WEBUI_FILTERS.md](guides/WEBUI_FILTERS.md) | WebUI filtre kullanımı |

## 📋 Son Eklenen Özellikler (v2.6.5.6+)

- **Yüklü ROM'ları Gizle Filtresi** - TVUI ve WebUI'da indirilmiş oyunları listeden gizleme
- **qBittorrent WebUI Şifre Yönetimi** - Varsayılan şifre uyarısı + değiştirme
- **Port Çakışma Yönetimi** - Port doluysa otomatik alternatif port
- **RGSX Download Manager** - Bağımsız arka plan indirme servisi
- **SSE Tabanlı Gerçek Zamanlı Güncelleme** - WebUI/TVUI canlı durum yansıması

## 🔗 Ana Kaynak Kod

- **TVUI (Pygame)**: `../ports/RGSX/display.py`, `controls.py`, `__main__.py`
- **WebUI (Flask)**: `../ports/RGSX/rgsx_web.py`, `static/js/app.js`
- **Download Manager**: `../ports/RGSX/rgsx_manager.py`
- **Ayarlar**: `../ports/RGSX/rgsx_settings.py`
- **İndirme Mantığı**: `../ports/RGSX/network.py`
- **Filtreler**: `../ports/RGSX/game_filters.py`
- **Çeviriler**: `../ports/RGSX/languages/*.json` (7 dil)

---

*Son güncelleme: 2026-08-07*