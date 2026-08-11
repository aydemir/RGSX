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
- Mevcut durum (2026.08): **364 test** — **341 passed / 23 pre-existing**
  (`test_display_core.py` 6, `test_display_filter.py` 15, `test_display_helpers.py` 2;
  display + pygame-stub ortamı, real pygame'de geçer). Baselines: 325 (Faz 8) → 341 (Faz 9).
- Gate modülleri %100: `game_filters.py`, `thread_safety.py`, `watchdog.py` (saf, headless).
- Toplam `TOTAL` satırı düşük görünür (%20) çünkü `.coveragerc` çoğu modülü `omit` eder ve
  ölçüme giren `tvui.py` (0%, 1365 ifade) ağır basar — kapsam tek başına kalite göstergesi değil.

## Kapsanan / kapsanmayan
- **Kapsanan (gate):** `game_filters.py`, `thread_safety.py` (%100), `watchdog.py` (%100),
  `display/__init__.py`, `display/filter.py`, `display/core.py`, `display/fonts.py`, `display/colors.py`.
- **Kısmi (yardımcı fonksiyon testleri var, modül düzeyinde kapsam düşük):**
  `display/{components,grid,game_list}` (saf yardımcılar: `fit_badge_lines`,
  `format_disk_size_gb`, `get_display_extension` vb.).
- **Ayrı testlenen ama cov gate'i dışı (`.coveragerc` `omit` `*/network/*`, `*/utils/*`):**
  `network/download_state.py` (`tests/test_download_state.py`, 57 test — Faz 8 state makinesi,
  illegal transition, backoff, history uyumu), `network/queue.py` batch akışı
  (`tests/test_download_batch.py`, 16 test — Faz 9, `_NoopThread` + `_SyncThread` ile).
  Kapsam raporuna dahil edilmek istenirse `omit` listesinden çıkarılabilir.
- **Hariç (roadmap):** `display/{menus,history,screens,icons,progress,scraper,
  support,text_viewer,virtual_keyboard,folder_browser,global_search,controls,
  background,transitions}` ve kalan altyapı modülleri.

## Regresyon notu
- `tests/test_display_filter.py` `config.game_filter_obj=None` iken
  `draw_filter_advanced`/`draw_filter_priority_config` çakmamasını garantiler
  (eski `hasattr` guard'ının ölü kod olması bug'ı — `config.py:519` ile birlikte düzeltildi).
