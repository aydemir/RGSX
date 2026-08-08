# Display Paketi Mimarisi

## Özet

`display.py` (6818 satırlık tekil dosya) kaldırıldı; yerine `ports/RGSX/display/` paketi oluşturuldu
(22 dosya: `__init__.py` + 21 modül). Amaç: God Object'i (SRP) küçük, tek sorumluluklu modüllere bölmek
(AGENTS.md 1. maddesi). Orijinal 94 fonksiyonun tamamı taşındı; davranış değişikliği yok.

## Modül haritası (satır ölçekleri)

| Modül | Rol | Satır |
|---|---|---|
| `__init__.py` | Public API yüzeyi: `__all__` (84 export), re-export'lar | 222 |
| `core.py` | Pencere init, metrik senkronizasyonu, paylaşılan `OVERLAY` | 201 |
| `menus.py` | Duraklatma/ayarlar/dil/görüntü menüleri + diyaloglar | 1731 |
| `history.py` | İndirme geçmişi ekranları | 802 |
| `grid.py` | Platform grid'i, header badge, disk satırı | 537 |
| `filter.py` | Filtre menüleri (seçim/avans/öncelik) + global sıralama | 475 |
| `game_list.py` | Oyun listesi + scrollbar + uzantı yardımcısı | 428 |
| `icons.py` | İkon satırı / footer kontrolleri | 310 |
| `controls.py` | Kontrol/IP/versiyon şeridi | 248 |
| `global_search.py` | Çapraz platform global arama listesi | 190 |
| `folder_browser.py` | Klasör gezgini | 189 |
| `components.py` | Buton, gölge, glow, header badge, uyarlanabilir layout | 180 |
| `screens.py` | Yükleme/hata/popup/toast ekranları | 178 |
| `scraper.py` | Scraper ekranı | 177 |
| `progress.py` | İndirme ilerleme ekranı | 133 |
| `text_viewer.py` | Metin dosyası görüntüleyici | 101 |
| `colors.py` | Tema renkleri + arka plan tema presetleri | 86 |
| `support.py` | Destek diyaloğu | 66 |
| `background.py` | Gradyan arka plan çizimi | 59 |
| `transitions.py` | Doğrulama geçiş animasyonu | 58 |
| `virtual_keyboard.py` | Sanal klavye | 38 |
| `fonts.py` | Badge font önbelleği | 27 |

## Kritik tasarım kararları

### 1. `OVERLAY` → `core.py`, erişim `get_overlay()`
- `OVERLAY` (ortak karartma yüzeyi) `core.py`'ye taşındı: `init_display()` /
  `sync_display_metrics()` tarafından oluşturulur.
- `get_overlay()` accessor'ı eklendi. **`display/core.py` dışında `OVERLAY`'e doğrudan
  atama yapılmaz** — `accessibility.py` ve `language.py` artık `get_overlay()` kullanıyor.
- Draw fonksiyonları modül içinden `core.OVERLAY` okur; testlerde dummy SDL ile
  doğrudan set edilebilir.

### 2. Public API'ye çevrilen 5 fonksiyon
Taşıma sırasında public API'ye (`display/__init__.py` `__all__`'ında) eklendi:
`get_badge_font`, `get_adaptive_badge_layout`, `fit_badge_lines`,
`format_disk_size_gb`, `render_combined_footer_controls`.

### 3. İçe aktarma disiplini
- Dış modüller (controls.py, __main__.py, rgsx_web.py) **yalnızca** `display/__init__.py`
  üzerinden içe aktarır (`from display import draw_filter_advanced` gibi).
- Modüller arası ortak erişim: `from .colors import THEME_COLORS`, `from . import core`.
- Çizim fonksiyonları `config.*` global state'ini okur (bu projenin mevcut deseni).

## Migration notları
- `import display` → `from display import X` (aynı).
- `display.OVERLAY` okuma → `from display import get_overlay` (yalnızca okuma).
- Doğrudan modül içi referans gerekiyorsa `display.core.OVERLAY`.

## Doğrulama
- `import display` + bağımlı modüller OK; SDL dummy ile ~19 ekran smoke testi geçti.
- **pytest altyapısı** (tests/): `display/` çekirdeği + `game_filters.py` +
  `thread_safety.py` için %97 toplam kapsam (bkz. `docs/guides/TESTING.md`).

## Gelecek
- `menus.py`/`history.py` gibi büyük çizim modülleri için per-modül stub test takımları
  (pygame yüzey mock'larıyla) eklenebilir — şu an `pytest.ini` `.coveragerc` omit'inden hariçtir.
