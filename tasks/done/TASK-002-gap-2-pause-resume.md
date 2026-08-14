# TASK-002-gap-2 — Pause/Resume Orkestrasyonu (Rust'ta eksik)

- **id:** TASK-002-gap-2
- **title:** Pause/Resume orchestration (toggle, pause_all, resume_all, pause_ev → backend)
- **status:** completed
- **priority:** P0
- **created:** 2026-08-14
- **completed:** 2026-08-14
- **environment:** both
- **tags:** pause, resume, systray, download
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `P0`, `P1`, `P2`, `P7`
- `ports/RGSX/network/queue.py`:
  - `toggle_pause_download` (satır 288) — event yoksa oluşturur, set/clear döndürür
  - `is_download_paused` (306), `pause_all_downloads` (333), `resume_all_downloads` (361),
    `is_any_download_paused` (387)
  - `pause_events` dict'i `qbittorrent_backend.download_torrent_via_qbittorrent`'e `pause_ev`
    olarak geçer; döngü içinde `pause_ev.is_set()` → qBittorrent API pause/resume (`P7`)
- Kullanıcı talebi (hafıza): systray "indirme duraklatma" özelliği bu düğüme bağlı

## Açıklama

Pause/resume şu an tamamen Python `pause_events` threading.Event sözlüğü üzerinden yürür.
qBittorrent path'inde `pause_ev` gerçek qBittorrent API pause/resume'e bağlanır (sadece
polling durmaz, torrent askıya alınır). Rust `LibrqbitEngine.download_torrent_source`
(`manager-torrent/src/lib.rs`) **pause/resume'i hiç desteklemiyor** — `wait_until_completed()`
ile bloklar, iptal/duraklatma sinyali yok. Systray duraklatma özelliği Rust torrent path'inde
çalışmaz.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — pause/resume kancaları (librqbit `ManagedTorrent` pause API)
- `manager-rs/manager-bin/src/` — `/api/pause`, `/api/resume` uçları + `pause_all`/`resume_all` toplu kontrol
- `rust_daemon.py` — `download_torrent` içinde `pause_ev` benzeri sinyalin Rust'a iletilmesi

## Uygulama Özeti

- `manager-bridge` — `TorrentBackend` trait'ine Gap-2 metotları (default = JSON-RPC proxy):
  `pause_all`/`resume_all` (P1/P2), `pause_torrent`/`resume_torrent` (P0), `is_paused`.
  `ProgressEvent`'e `paused: bool` alanı eklendi.
- `manager-torrent` — `LibrqbitEngine.active_handles: RwLock<HashMap<task_id, Arc<ManagedTorrent>>>`
  kaydı; `download_torrent_source_with_progress`'e `task_id` parametresi (handle'ı kaydeder,
  bitince/hatada kaldırır). `pause_active`/`resume_active` (tümü) + `pause_task`/`resume_task`
  (tekil) librqbit `Session::pause`/`unpause` (8.1.1) üzerinden. `call()`'a `pause_all`/
  `resume_all`/`pause`/`resume`/`is_paused` JSON-RPC metotları eklendi. Progress döngüsü
  `TorrentStatsState::Paused` → `paused: true` + speed 0 raporlar.
- `manager-http` — `/api/download` gövdesinden `task_id` okur ve engine'e iletir; `/api/pause`/
  `/api/resume` catalog proxy'sini korur, yoksa bridge'e `pause_all`/`resume_all` (task_id
  varsa tekil), bridge yoksa placeholder. Progress callback'i `Paused` durumunu WebUI'ye yazar.
- `rust_daemon.py` — `download_torrent(...)` imzasına `pause_ev`; poll döngüsünde set → `/api/pause`,
  clear → `/api/resume` (task_id ile). `/api/download` gövdesine `task_id` ekler.
- `queue.py` — Rust delegasyon çağrısına `pause_ev=pause_events.setdefault(task_id, Event())` geçer.

## Doğrulama

- `toggle_pause_download` davranışı (event oluşturma + set/clear dönüşü) Rust daemon'da yeniden üretilir.
- `pause_all_downloads`/`resume_all_downloads`: aktif torrentler + history durumu senkron güncellenir.
- qBittorrent fallback path'inde mevcut `P7` davranışı korunur (regression yok).
- Systray "duraklat" tıklanınca Rust torrenti gerçekten askıya alır (test: speed → 0).

### Testler

- `manager-torrent/tests/engine.rs` — +4 birim (pause_all/resume_all boş map → 0, bilinmeyen
  task no-op, `call` dispatch `{paused:0}`/`{resumed:0}`/`is_paused:false`). 13 geçiyor.
- `manager-http/tests/contract.rs` — 103 test geçiyor (test_pause/test_resume/test_pause_proxied/
  test_resume_proxied dahil; `Json<Option<Value>>` extractor'ı ile placeholder korunur).
- `tests/test_rust_daemon.py` — +3 (body'de task_id, pause/resume döngüsü, pause_ev yokken POST yok).
  Sandbox'ta pygame eksik → import-stub ile manuel doğrulandı; tam pytest tamamlanamadı (env).
- Çevrimdışı birim testlerde session spawn tetiklenmemesi için `session_handles` boş map'te
  `ensure_running`'e gitmez (DHT persistent kurulumu çevrimdışı başarısız olurdu).

### Ek düzeltmeler (pre-existing, build'i açmak için)

- `manager-http/src/catalog_bootstrap.rs` — Linux'ta `Permissions::from_mode` için eksik
  `PermissionsExt` import'u eklendi.
- `manager-http/Cargo.toml` — `zip` native (bzip2/lzma/zstd-sys) bağımlılıklarını kaldırmak için
  `default-features = false, features = ["deflate"]` (Termux ARM liblzma link çakışması; games.zip
  okuması deflate ile çalışır).
