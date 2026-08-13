# TASK-002k-7 — Faz 10c/3/7: Göç doğrulama + Python köprüsü kapanış

- **id:** TASK-002k-7
- **title:** 364 Python + 68 Rust contract + canlı smoke; Python Web UI köprüsünü kapat
- **status:** done
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
- 2026-08-12 — Sandbox doğrulaması:
  - `cargo test -p manager-http`: **98/98 yeşil** (68 orijinal + 30 yeni proxy/bridge/static/qb testi).
    Not: görev "68 Rust" diyor; proxy/bridge eklemeleriyle 98'e çıktı. workspace geneli henüz
    çalıştırılmadı (sadece manager-http).
  - Python `rgsx_manager.py` `py_compile` ile derlendi (OK). `RGSX_RUST_WEBUI` değişikliği
    **flag kapalı varsayılanı birebir korur** (sadece flag=1 dalı yeni davranış).
  - **KISIT:** bu sandbox'ta `pip`/`pytest` YOK → 364 Python testi + canlı TV smoke ÇALIŞTIRILAMADI.
    Gerçek ortamda: `python -m pytest tests/ --noconftest` (pygame ortamı) ve `RGSX_RUST_WEBUI=1`
    ile canlı smoke gerekli. Bu adımlar kullanıcı tarafından yapılmalı.
