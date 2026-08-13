# TASK-002i — Faz 10c: Rust manager-bin'i gerçek daemon yap

- **id:** TASK-002i
- **title:** Rust manager-bin'i uygulamanın gerçek torrent daemon'ı yap (Python rgsx_manager.py yerine/augment)
- **status:** done
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, manager-bin, daemon, faz-10c, entegrasyon
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10 (Rust kısmi refaktör).
- TASK-002f (librqbit engine + Rust-HTTP /api/download), TASK-002g (canlı doğrulama, librqbit
  varsayılan), TASK-002h (Windows cross-compile, varsayılan yapıldı) ✅.
- Kullanıcı onayı (2026-08-12, "başla"): Faz 10c'yi başlat.

## Açıklama (Faz 10c — aşamalı plan)

Şu an `manager-bin` bağımsız HTTP sunucu; Python `rgsx_manager.py` + `rgsx_web` ayrı çalışıyor,
Rust binary uygulamaya bağlı DEĞİL. Keşif (codegraph, 2026-08-12) bulguları:
- Python `rgsx_manager.py::main()` Web UI'yi `rgsx_web.run_server(ManagerHandler)` + queue
  worker + SSE broadcaster + watchdog + tray ile çalıştırır (port `get_manager_port()`, önt. 5000).
- TV UI `__main__.py` → `manager_launcher.ensure_manager()` `rgsx_manager.py`'yi subprocess
  spawn edip supervise eder; sağlıklı olunca `config.manager_available=True`.
- Rust `manager-http::router()` TÜM route yüzeyini exposure eder (platforms/search/games/
  settings/support/qbittorrent/*/download/events…) AMA çoğu handler **placeholder**
  (boş/statik); yalnız `/api/download` (torrent) + `/api/health` + SSE fonksiyonel. Rust portu
  5010 (`RGSX_MANAGER_BIN_PORT`), Python'dan ayrı.

**Aşamalar:**
- **Faz 10c/1 (bu görev):** Python `rgsx_manager.py` (veya `manager_launcher`), Rust `manager-bin`'i
  **supervised sidecar torrent daemon** olarak spawn eder + sağlığını izler + durumunu Web UI'ye
  yansıtır. Flag/kapalı: `RGSX_RUST_DAEMON` env; yalnız binary mevcutsa. İndirme akışı DEĞİŞMEZ
  (risk sıfır, 364 Python testi korunur). Temel bağlama (spawn+supervise+health) kanıtlanır.
- **Faz 10c/2:** Python `_api_download`/download worker, Rust daemon sağlıklıyken torrent
  indirmeyi `http://127.0.0.1:5010/api/download`'a devreder (SSE ile izler); aksi halde mevcut
  Python yoluna fallback. Opt-in flag.
- **Faz 10c/3 (ileride, büyük):** Rust handler'ların gerçek mantığını yaz (games/search/settings/
  support...) ve Web UI sunucusunu Rust'e çevir. Ayrı devasa görev.

**Bu görevde kapsam: Faz 10c/1 (sidecar spawn + supervise + health-expose).**

**Korunması gereken contract'lar:**
- Web UI (`rgsx_web` static + `/api/*`) davranışı flag kapalıyken birebir aynı kalmalı.
- SSE akışı kesintisiz.
- qBittorrent WebUI (Python bridge opt-in) hâlâ erişilebilir.
- 364 Python testi + 114 Rust testi yeşil kalmalı.

## Kapsam / Dosyalar

- `ports/RGSX/rgsx_manager.py` — daemon başlatma, supervisor (sidecar spawn eklenecek)
- `ports/RGSX/manager_launcher.py` — `ensure_manager` / supervisor loop (health izleme)
- `manager-rs/manager-bin/src/main.rs` — engine + HTTP (değişmez, sadece tüketilir)
- `manager-rs/manager-http/src/api.rs` — health/status (durum yansıması için)

## Doğrulama

- Python manager, Rust `manager-bin`'i başlatıp sağlığını poll eder; torrent istekleri Rust'a düşer.
- Mevcut Python test paketi (364 test) davranışı korur.
- `cargo test --workspace` + canlı smoke yeşil.

## İlerleme

- 2026-08-12 — Tanımlandı (kullanıcı "başla"). Keşif/plan aşaması.
- 2026-08-12 — **Faz 10c/1 TAMAMLANDI (kod + test):**
  - `ports/RGSX/rust_daemon.py` (YENİ): flag-gated sidecar süpervizörü — `RGSX_RUST_DAEMON=1`
    ve binary mevcutsa `manager-bin`'i subprocess spawn eder (port 5010), `/api/health` poll
    eden daemon thread ile `watchdog.RestartLimiter` (1sa/3restart) sınırlı yeniden başlatır;
    durum `config.rust_daemon_available`'a yazılır. Flag kapalı/binary yok → no-op (Python-only).
  - `ports/RGSX/config.py`: `rust_daemon_available = False` eklendi.
  - `ports/RGSX/rgsx_manager.py::main()`: tray kurulumundan sonra sidecar başlatma +
    süpervizör thread'i bağlandı (hata yutularak güvenli).
  - `tests/test_rust_daemon.py` (YENİ): 6 test — enabled flag, _resolve_bin, start (disabled/
    enabled/missing-binary), healthy. `--noconftest` ile geçti (6 passed).
  - `docs/PROJECT_MAP.md` güncellendi (Faz 10c/1 köprüsü notu).
  - **Sonraki:** Faz 10c/2 (Python indirme akışını Rust daemon'a devretme, opt-in) ayrı görev.
