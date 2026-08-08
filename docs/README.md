# RGSX Documentation

Bu klasör RGSX projesinin **geliştirici dokümantasyonunu** içerir. Kullanıcı kılavuzları
`docs/user/` altında ayrılmıştır.

## 📁 Klasör Yapısı

```
docs/
├── README.md                      # Bu dosya - dokümantasyon indeksi
├── architecture/                  # Mimari ve tasarım
│   ├── DOWNLOAD_MANAGER.md        # RGSX Download Manager daemon mimarisi
│   └── DISPLAY_PACKAGE.md         # display/ paketi mimarisi (display.py bölünmesi)
├── flows/                         # Kritik çalışma akışları (kod doğrulamalı)
│   ├── STARTUP.md                 # Başlatma: ensure_manager + SSE yansıması + çift manager
│   ├── DOWNLOAD_PIPELINE.md       # İndirme: HTTP resume + torrent + kuyruk worker
│   └── FILTER_PIPELINE.md         # Filtre: GameFilters modeli + TVUI/WebUI uygulaması
├── guides/                        # Geliştirici kılavuzları
│   ├── DEVELOPMENT.md             # Ortam kurulumu + değişiklik döngüsü + commit/push
│   └── TESTING.md                 # pytest altyapısı, kapsam gate'leri, SDL izolasyonu
├── features/
│   └── FEATURES.md                # Özellikler ve değişiklik günlüğü (changelog)
├── roadmap/
│   └── ROADMAP.md                 # Geliştirme yol haritası (tüm fazlar tamamlandı)
├── user/                          # Kullanıcı kılavuzları
│   ├── TVUI_FILTERS.md            # TVUI filtre kullanım kılavuzu
│   └── WEBUI_FILTERS.md           # WebUI filtre kullanım kılavuzu
└── deprecated/                    # Tarihsel/kaldırılmış belgeler
    ├── FOCUS_FIX.md               # Kaldırılan odak düzeltmesi (referans)
    └── ES_INTEGRATION_ANALYSIS.md # ES entegrasyon analizi (manager öncesi, tarihsel)
```

## 🚀 Hızlı Başlangıç (Developer)

| Doküman | Açıklama |
|---------|----------|
| [DEVELOPMENT.md](guides/DEVELOPMENT.md) | **Önce bunu oku** — ortam, değişiklik döngüsü, commit/deploy |
| [STARTUP.md](flows/STARTUP.md) | TVUI + manager başlatma ve SSE delegasyonu |
| [DOWNLOAD_PIPELINE.md](flows/DOWNLOAD_PIPELINE.md) | İndirme akışı: HTTP resume + torrent + kuyruk |
| [FILTER_PIPELINE.md](flows/FILTER_PIPELINE.md) | Filtre modeli ve TVUI/WebUI uygulaması |
| [DOWNLOAD_MANAGER.md](architecture/DOWNLOAD_MANAGER.md) | Manager daemon mimarisi |
| [DISPLAY_PACKAGE.md](architecture/DISPLAY_PACKAGE.md) | display/ paketi mimarisi |
| [TESTING.md](guides/TESTING.md) | pytest altyapısı ve kapsam gate'leri |
| [FEATURES.md](features/FEATURES.md) | Tüm özellikler ve sürüm notları |
| [ROADMAP.md](roadmap/ROADMAP.md) | Yol haritası (Faz 1-7 tamamlandı) |

## 🧭 Kullanıcı Kılavuzları

| Doküman | Açıklama |
|---------|----------|
| [TVUI_FILTERS.md](user/TVUI_FILTERS.md) | TVUI filtre kullanımı |
| [WEBUI_FILTERS.md](user/WEBUI_FILTERS.md) | WebUI filtre kullanımı |

## 🔗 Ana Kaynak Kod

- **TVUI (Pygame)**: `../ports/RGSX/display/` (paket), `controls.py`, `__main__.py`
- **WebUI**: `../ports/RGSX/rgsx_web.py` (`BaseHTTPRequestHandler`), `static/js/app.js`
- **Download Manager**: `../ports/RGSX/rgsx_manager.py`
- **Ayarlar**: `../ports/RGSX/rgsx_settings.py`
- **İndirme Mantığı**: `../ports/RGSX/network.py`
- **Filtreler**: `../ports/RGSX/game_filters.py`
- **Thread Güvenliği**: `../ports/RGSX/thread_safety.py`
- **Çeviriler**: `../ports/RGSX/languages/*.json` (7 dil)

---

*Son güncelleme: 2026-08-08 (developer dokümantasyonu düzeni)*
