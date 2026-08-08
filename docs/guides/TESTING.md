# RGSX Test Altyapısı (pytest)

## Kurulum
- Bağımlılıklar: `pytest` (9.x), `pytest-cov` (coverage ile birlikte), `pygame-ce`.
- Kurulum: `pip install pytest-cov` (coverage dahil gelir).

## Çalıştırma
```bash
pytest                                    # varsayılan (pytest.ini kapsamı)
pytest --cov=. --cov-report=term-missing  # tüm proje + tests (AGENTS.md komutu)
```

## Yapılandırma
- `pytest.ini`: `testpaths=tests`, `pythonpath=ports/RGSX`,
  `addopts = --cov-config=.coveragerc --cov=ports/RGSX --cov-report=term-missing`.
- `.coveragerc`: `omit` — kapsam dışı modüller (ağır pygame çizim modülleri, altyapı,
  web/manager/network). Bu, kapsam sayacını **refaktör edilen çekirdeğe** odaklar.

## SDL izolasyonu (headless)
- `tests/conftest.py` `pygame` import edilmeden ÖNCE `SDL_VIDEODRIVER=dummy` +
  `SDL_AUDIODRIVER=dummy` ortam değişkenlerini set eder → tüm çizim kodu penceresiz çalışır.
- `display_env` fixture: minimal config stub'ları (`screen_width/height`, `font`,
  `title_font`, `small_font`, `tiny_font`, `screen`, `core.OVERLAY`) kurar ve test
  sonunda `config.game_filter_obj`'u sıfırlar.

## Kapsam hedefi
- Hedef: refaktör edilen modüllerde **%80+** (AGENTS.md 3. maddesi).
- Şu anki rapor: `display/{__init__,colors,core,filter,fonts}`, `game_filters.py`,
  `thread_safety.py` → **%95 toplam** (151 test).

## Kapsanan / kapsanmayan
- **Kapsanan (gate):** `game_filters.py`, `thread_safety.py`, `display/__init__.py`,
  `display/filter.py`, `display/core.py`, `display/fonts.py`, `display/colors.py`.
- **Kısmi (yardımcı fonksiyon testleri var, modül düzeyinde kapsam düşük):**
  `display/{components,grid,game_list}` (saf yardımcılar: `fit_badge_lines`,
  `format_disk_size_gb`, `get_display_extension` vb.).
- **Hariç (roadmap):** `display/{menus,history,screens,icons,progress,scraper,
  support,text_viewer,virtual_keyboard,folder_browser,global_search,controls,
  background,transitions}` ve tüm altyapı/web/network modülleri.

## Regresyon notu
- `tests/test_display_filter.py` `config.game_filter_obj=None` iken
  `draw_filter_advanced`/`draw_filter_priority_config` çakmamasını garantiler
  (eski `hasattr` guard'ının ölü kod olması bug'ı — `config.py:519` ile birlikte düzeltildi).
