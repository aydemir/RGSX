# TASK-002k-7 — Faz 10c/3/7: Göç doğrulama + Python köprüsü kapanış

- **id:** TASK-002k-7
- **title:** 364 Python + 68 Rust contract + canlı smoke; Python Web UI köprüsünü kapat
- **status:** todo
- **priority:** P2
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, doğrulama, göç
- **parent:** TASK-002k

## Açıklama

Tüm 10c/3 alt görevleri sonrası bütünleşik doğrulama:
- `cargo test --workspace` (114 Rust test) + `python -m pytest tests/ --noconftest` (364 Python, pygame
  ortamında) yeşil.
- `manager-http/tests/contract.rs` TÜM route için assertion içerir (boşluk kalmaz).
- Canlı smoke: TV UI + Tarayıcı, `RGSX_RUST_WEBUI=1` ile uçtan uca (indirme, SSE, settings, support).
- Python `rgsx_web` köprüsü yalnız fallback olarak bırakılır; varsayılan Rust Web UI olur.

## Kapsam / Dosyalar

- Test paketi + `docs/PROJECT_MAP.md` (10c/3 durumu), `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md`.

## Doğrulama

- Tüm testler yeşil; canlı smoke kesintisiz; `RGSX_RUST_WEBUI` varsayılanı belgelenir.

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
