# TASK-002j — Faz 10c/2: Python torrent indirme akışını Rust daemon'a devret

- **id:** TASK-002j
- **title:** Python `_api_download`/download worker, Rust daemon sağlıklıyken torrent indirmeyi `http://127.0.0.1:5010/api/download`'a devreder (SSE/poll ile izler); aksi halde mevcut Python qBittorrent yoluna fallback
- **status:** in-progress
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, manager-bin, daemon, faz-10c, torrent, entegrasyon
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10c.
- TASK-002i (Faz 10c/1) ✅ — `rust_daemon.py` sidecar süpervizörü + `config.rust_daemon_available`.
- Kullanıcı onayı (2026-08-12, "başla"): Faz 10c/2'yi başlat.

## Açıklama (Faz 10c/2)

`download_rom` torrent dalı (`torrent_meta is not None`) şu an yalnız `qbittorrent_backend.download_torrent_via_qbittorrent` çağırır. Faz 10c/2: Rust `manager-bin` (librqbit) sağlıklı ve opt-in `RGSX_RUST_TORRENT=1` set ise, torrent indirme `http://127.0.0.1:5010/api/download`'a devredilir; ilerleme `/api/progress` poll edilip Python `config.history`/`config.download_progress`'a yansıtılır; iptal `cancel_ev` ile izlenir. Herhangi bir hata/timeout → mevcut qBittorrent yoluna **fallback** (risk sıfır).

**Sözleşme notları (codegraph + api.rs doğrulandı):**
- Rust `POST /api/download` `{platform, game_name, url}` alır; bridge varsa arka planda `download_torrent(url, dest_path)` koşar, bitince `finalize_download_in_state` ile kendi state'ine yazar. Yanıt `queued:true` + `task_id` (block etmez).
- Rust `dest_path` normalde `downloads_folder`+türetilen ad; **Faz 10c/2 Rust değişikliği:** gövdede isteğe bağlı `dest_path` kabul eder (Python kendi hedefini verir → postprocess bozulmaz). `RGSX_DOWNLOADS_FOLDER` env ile librqbit output klasörü `config.ROMS_FOLDER`'a çekilir.
- Rust `GET /api/progress` → `{downloads: {url: {status, progress}}}`; `GET /api/health` → `{success, manager}`. `/api/cancel` placeholder (geri dönüş mesajı) — cancel yerelde `cancel_ev` ile ele alınır.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` — `download` handler: isteğe bağlı `dest_path` (geriye dönük uyumlu).
- `ports/RGSX/rust_daemon.py` — `RUST_TORRENT_ENABLED()`, `start()`'ta `RGSX_DOWNLOADS_FOLDER`, `download_torrent(...)` yardımcı.
- `ports/RGSX/network/queue.py` — `download_rom` torrent dalı: devir + fallback.
- `tests/test_rust_daemon.py` — `download_torrent` birim testi (mock HTTP).
- `docs/PROJECT_MAP.md` — Faz 10c/2 notu.

## Doğrulama

- `RGSX_RUST_TORRENT=0` (varsayılan): davranış birebir aynı (364 Python testi yeşil).
- `RGSX_RUST_TORRENT=1` + binary + healthy: torrent `manager-bin`'e düşer; ilerleme Web UI'ye yansır; çökme/timeout → qBittorrent'e fallback.
- `cargo test -p manager-http` + `python -m pytest tests/test_rust_daemon.py --noconftest` yeşil.

## İlerleme

- 2026-08-12 — Tanımlandı ("başla"). Araştırma + plan: Rust `/api/download`/`/api/progress`/`/api/cancel` ve `download_rom` torrent dalı codegraph ile doğrulandı.
- 2026-08-12 — Uygulama: Rust `dest_path` kabulü + `rust_daemon.download_torrent` + `download_rom` devir/fallback + test.
- 2026-08-12 — **TAMAMLANDI (kod + test):** `cargo build -p manager-http` + `cargo test -p manager-http` (68 contract test yeşil); `pytest tests/test_rust_daemon.py --noconftest` (11 test yeşil). `py_compile` temiz. `docs/PROJECT_MAP.md` güncellendi. Varsayılan `RGSX_RUST_TORRENT` kapalı → mevcut davranış birebir korunur (pygame yok → tam 364 test bu ortamda koşulamadı, devir dalı guard'lı).
