# Kritik Akış: Filtre Pipeline'ı (TVUI + WebUI)

> Geliştirici notu: Satır referansları commit `c5c5685` (Faz 9) itibarıyla geçerlidir.
> Modüller: `game_filters.py`, `display/filter.py`, `controls/search.py`,
> `controls/handlers.py`, `rgsx_web/handlers.py`, `static/js/app.js`.

## Özet

Filtreler tek bir kaynakta tanımlıdır: `GameFilters` sınıfı (`game_filters.py`).
TVUI ve WebUI **aynı modeli** paylaşır; arayüzler `rgsx_settings.json` → `game_filters`
anahtarında birleşir. `GameFilters` saf (I/O'suz) bir sınıftır — bu yüzden test altyapısında
ilk %98 kapsam hedefiydi.

```
rgsx_settings.json ── game_filters ──┐
                                     ├──► GameFilters (game_filters.py)
TVUI: controls/search.py + display/filter.py çizim ──┘   │
WebUI: /api/settings + /api/save_filters + app.js        │
                                     └──► apply_filters(games, platform_name)
```

## 1. Model: `GameFilters` (game_filters.py:27)

Durum alanları:

| Alan | Varsayılan | Açıklama |
|---|---|---|
| `region_filters` | `{}` | `{bölge: "include"\|"exclude"\|"none"}` — 9 bölge |
| `hide_non_release` | `False` | `[Demo]/[Beta]/[Proto]` gizle |
| `one_rom_per_game` | `False` | Baş oyun başına tek ROM (bölge önceliğine göre) |
| `hide_downloaded` | `False` | HDD'de yüklü oyunları gizle |
| `regex_mode` | `False` | WebUI arama regex |
| `region_priority` | `[USA, Canada, World, Europe, Japan, Other]` | `one_rom_per_game` sıralaması |

Serileştirme: `load_from_dict` (game_filters.py:42), `to_dict` (56), `is_active` (67),
`reset` (75). Kalıcılık `rgsx_settings.json` `game_filters` anahtarında.

Bölge tespiti yardımcıları (statik, önbellekli):
- `get_game_regions(game_name)` (84) — dosya adındaki bölge etiketlerini parse eder.
- `is_non_release_game` (159), `get_base_game_name` (181) — demo/beta/proto ve base ad.
- `get_cached_*` (211/218/225) — `functools.lru_cache` üzerinden tekrarlı hesaplamayı önler.
- `get_region_priority` (231) — oyunun bölgesine göre öncelik puanı.

## 2. Uygulama: `apply_filters` (game_filters.py:249)

Sıralı pipeline:

1. `is_active()` False → orijinal listeyi döndür (hızlı yol).
2. `hide_downloaded` ise `platform_name` çöz (`_resolve_platform_name` fallback).
3. **Bölge:** herhangi `exclude` varsa, oyunun hiçbir bölgesi `include` değilse oyunu atla.
4. **non-release:** `hide_non_release` → `get_cached_non_release` True ise atla.
5. **hide_downloaded:** `history.is_game_downloaded(platform_name, game.name)` True ise atla.
6. **one_rom_per_game:** `_apply_one_rom_per_game` (306) — `get_base_game_name` ile grupla,
   grupta tek ROM varsa bırak, çoksa `get_region_priority` ile sıralayıp en yükseği seç.

## 3. TVUI akışı

### Menü / input (controls/ paketi)

- Filtre menüsü açıldığında `config.game_filter_obj` üzerinde `region_filters`,
  `hide_non_release`, `one_rom_per_game`, `hide_downloaded`, `region_priority` güncellenir.
- `filter_games_by_search_query()` (controls/search.py:33): `game_filter_obj.is_active()` ise
  `apply_filters(config.games, platform_name)` → sonrasında `search_query` substring filtresi
  + `_sort_local_games` sıralama.
- "Uygula" → `to_dict()` → `rgsx_settings` kaydet + `config.needs_redraw=True`.
- İndirme menüsü girişi `controls/handlers.py` dispatch'i üzerinden; filtre menüleri
  `controls/menus.py`'de (`VALID_STATES`/`validate_menu_state`).

### Çizim (display/filter.py)

- `draw_filter_advanced` (display/filter.py:120) — 3×3 bölge grid + "Yüklü ROM'ları Gizle"
  toggle + `one_rom_per_game` + öncelik sırası satırları.
- `draw_filter_priority_config` — bölge öncelik sırası ekranı.

**Regresyon uyarısı (bug fix b874b99):** Bu fonksiyonlar `config.game_filter_obj`'u
**guard'layarak** kullanır. `config.py:519` `game_filter_obj` başlangıçta `None` tanımlar;
eski `if not hasattr(config, 'game_filter_obj'):` guard'ı ölü koddur (`hasattr` daima True)
ve `None.region_filters` ile `AttributeError` verirdi. Doğru desen:

```python
if config.game_filter_obj is None:
    return  # veya default çizim
```

Aynı koruma `controls/` (x3) ve `rgsx_web/handlers.py` (`getattr(...) is not None`) için de geçerli.
`tests/test_display_filter.py` bu regresyonu tutar.

## 4. WebUI akışı

### API (rgsx_web/handlers.py + handlers_settings.py)

| Endpoint | Rol |
|---|---|
| `GET /api/settings` | Tüm ayarlar (game_filters dahil) → `loadSavedFilters()` |
| `POST /api/save_filters` | Sadece filtreleri kaydet → `config.game_filter_obj` güncelle |
| `POST /api/settings` | Tüm ayarlar (game_filters dahil) |

`POST /api/save_filters` payload:

```json
{
  "region_filters": {"USA": "include", "Japan": "exclude"},
  "hide_non_release": true,
  "one_rom_per_game": false,
  "hide_downloaded": true,
  "regex_mode": false,
  "region_priority": ["USA", "Canada", "World", "Europe", "Japan", "Other"]
}
```

`rgsx_web/handlers.py` tarafında da aynı `config.game_filter_obj is None` guard'ı gereklidir
(server-side render sırasında obj henüz init edilmemiş olabilir).

### Frontend (static/js/app.js)

- `loadSavedFilters()` (DOMContentLoaded) → checkbox/durumları set eder.
- `applyAllFilters()` → item bazında client-side filtreleme:
  - bölge: `item.dataset.regions` (CSV) ↔ include/exclude
  - `hide_downloaded`: `item.dataset.downloaded === 'true'`
  - `hide_non_release`, `one_rom_per_game`, regex arama.
- Her değişiklik anlık uygulanır + `saveFiltersToBackend()` → `POST /api/save_filters`.
- Checkbox'lar `??` operatörü ile okunur (`checked` yanlış pozitif/negatif handling —
  `c9a6492` bug fix).

## 5. Bölge seti ve çeviriler

| Bölge | TVUI | WebUI |
|---|---|---|
| USA, Canada, Europe, France, Germany, Japan, Korea, World, Other | `region_filters` 3×3 grid | `region_filters` dropdown/pill |
| "Yüklü ROM'ları Gizle" | `filter_hide_downloaded` (7 dil) | `web_filter_hide_downloaded` (7 dil) |

Çeviriler `languages/*.json`'da; TVUI `_()` / WebUI `t()` üzerinden okunur.

## İlgili dosyalar

- `game_filters.py` (model + `apply_filters`)
- `display/filter.py` (`draw_filter_advanced`, `draw_filter_priority_config`)
- `controls/search.py` (`filter_games_by_search_query`), `controls/menus.py` (filtre menüleri),
  `controls/handlers.py` (dispatch)
- `rgsx_web/handlers.py` (`/api/save_filters`, `/api/settings`)
- `static/js/app.js` (`applyAllFilters`, `loadSavedFilters`, `saveFiltersToBackend`)
- `tests/test_display_filter.py` + `tests/test_game_filters.py` (regresyon)
