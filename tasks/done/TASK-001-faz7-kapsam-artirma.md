# TASK-001 — Faz 7 kapsam artırma (rgsx_settings / rgsx_manager / qbittorrent_backend)

- **id:** TASK-001
- **title:** Faz 7 kapsam artırma — rgsx_settings, rgsx_manager, qbittorrent_backend
- **status:** done
- **updated:** 2026-08-11
- **priority:** P2
- **created:** 2026-08-11
- **environment:** linux
- **tags:** tests, coverage, rgsx-settings, rgsx-manager, qbittorrent-backend

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 7 (Test altyapısı: characterization tests, "✅ TAMAMLANDI" sonrası ertelenen hedef)

## Açıklama

Roadmap Faz 7, `.coveragerc`'ten `rgsx_web/`, `rgsx_settings.py`, `rgsx_manager.py` ve
`qbittorrent_backend.py` için omit'i kaldırıp bu dosyaları ölçüme aldı ve davranışı sabitleyen
request-level contract testlerini ekledi (asıl amacı bitti). Ancak Faz 7 bölümünün sonunda açıkça
şu hedef yazıyor: **"`rgsx_settings.py` / `rgsx_manager.py` / `qbittorrent_backend.py` için hedef
bir sonraki Faz 7 iterasyonunda daha fazla testle artırılacak."**

Faz 7'nin bitişindeki kapsam ölçümü (ilk gerçek sayılar):

| Dosya | Kapsam |
|---|---|
| `rgsx_web/handlers.py` | 76% |
| `rgsx_web/handlers_settings.py` | 62% |
| `rgsx_web/handlers_games.py` | 58% |
| `rgsx_web/__init__.py` + `cache.py` | 55% |
| `rgsx_web/handlers_ui.py` | 48% |
| `rgsx_web/handlers_download.py` | 36% |
| `rgsx_web/i18n.py` | 30% |
| `rgsx_web/server.py` | 10% |
| `rgsx_manager.py` | 31% |
| `rgsx_settings.py` | 23% |
| `qbittorrent_backend.py` | 20% |
| **TOTAL** | 18% (9538 stmt) |

Aradan geçen fazlarda bu hedefe kısmen katkı oldu: `tests/test_password_migration.py` (21 test,
Faz 5) ve `tests/test_qbittorrent_port.py` (14 test, Faz 3) `qbittorrent_backend.py` +
`rgsx_settings.py`'i, `tests/test_qbittorrent_backend.py` (23 test) backend'i, `tests/test_api_contract.py`
(54 test) WebUI + manager yüzeyini kapsıyor. Yine de üç öncelikli modülün kapsamı (%31/%23/%20)
resmi olarak hedeflenmiş bir seviyeye çekilmedi; artış izlenmiyor.

Görevin kapsamı: üç öncelikli modül (`rgsx_settings.py`, `rgsx_manager.py`,
`qbittorrent_backend.py`) için hedeflenen kapsam seviyesini belirlemek ve davranışı sabitleyen
testlerle oraya taşımak; `.coveragerc`'te bu dosyaların omit'li **kalmadığını** korumak.

**Hedef kapsam (2026-08-11 onayı):** `rgsx_settings.py` ≥ %60, `rgsx_manager.py` ≥ %55,
`qbittorrent_backend.py` ≥ %45. Baz: settings %26 / manager %30 / qbittorrent %22
(341 passed / 23 pre-existing display fail).

## Kapsam / Dosyalar

- `tests/test_qbittorrent_backend.py` — backend davranış testleri
- `tests/test_qbittorrent_port.py` — port fallback testleri
- `tests/test_password_migration.py` — şifre migration testleri
- `tests/test_api_contract.py` — WebUI + manager endpoint sözleşme testleri
- Yeni test dosyaları (rgsx_settings persister'ları, rgsx_manager endpoint'leri için)
- `.coveragerc` — omit listesi korunur (yeni geri ekleme yok)

## Doğrulama

- `RGSX_HEADLESS=1 PYTHONPATH=/tmp/pygame_stub python -m pytest tests/ -q` tam geçer;
  pre-existing display/pygame-stub grubu (23) değişmez.
- `pytest --cov=. --cov-report=term-missing` ile üç hedef modülde ölçülebilir kapsam artışı
  ve her yeni testin en az bir önceden ölçümsüz dalı/branch'ı kapsadığı doğrulanır.
- Dev makinesinde canlı: `rgsx_manager` endpoint'leri (`/api/health`, `/api/qbittorrent/*`,
  `/api/shutdown`, `/api/pause`/`/api/resume`) gerçek süreç üzerinde smoke test edilir.

---

## İlerleme

- 2026-08-11 — Roadmap'ten tasks/ yapısına taşındı (in-progress; kısmen ilerlemiş: qbittorrent/
  rgsx_settings için ek testler mevcut, resmi kapsam hedefi henüz tanımlı değil).
- 2026-08-11 — **TAMAMLANDI.** Üç hedef modül de hedefin üzerine taşındı:
  `rgsx_settings.py` %94 (hedef ≥60), `rgsx_manager.py` %88 (hedef ≥55),
  `qbittorrent_backend.py` %80 (hedef ≥45). Test dosyaları: `tests/test_rgsx_settings.py`
  (870 satır), `tests/test_rgsx_manager.py` (1151 satır), `tests/test_qbittorrent_backend.py`
  (194+ test; login/ensure/download/seed akışları). Düzeltilen gerçek platform hatası:
  `os.link` Termux/Android'de `AttributeError` fırlatıyordu → `except (OSError, AttributeError)`
  ile `shutil.copy2` fallback'i çalışır hale geldi (ports/RGSX/qbittorrent_backend.py:1682).
  Tam suite: 725 passed / 23 pre-existing display fail (baseline korundu).
  Commit: `ca14a3f`.
