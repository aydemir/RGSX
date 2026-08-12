# TASK-002k — Faz 10c/3 planı: Rust handler'ların gerçek mantığı + Web UI sunucusunu Rust'e taşıma

- **id:** TASK-002k
- **title:** Faz 10c/3 — Rust `manager-http` placeholder handler'larını doldur ve Python Web UI sunucusunu Rust'e göç ettir (alt görev planı)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, webui, göç, plan
- **parent:** TASK-002

## Kaynak

- TASK-002i:40 — Faz 10c/3 tek satır tanım ("Rust handler'ların gerçek mantığı + Web UI sunucusunu Rust'e çevir. Ayrı devasa görev").
- `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` Faz 10 — sözleşme: `/api/*` + `/api/events` (SSE) birebir korunur; Faz 7 characterization testleri garanti.
- Keşif (codegraph + `manager-rs/manager-http/src/api.rs` okuma, 2026-08-12).

## Kök neden / Davranış kuralları (araştırma)

**Mevcut durum:** Rust `manager-bin` (port 5010) şu an yalnız torrent daemon'u (Faz 10c/1/2).
`manager-http` route yüzeyi geniş ama handler'ların çoğu **placeholder** (boş/statik yanıt).
Gerçek iş mantığı hâlâ Python `rgsx_web` + `controls` + `network` + `rgsx_settings` + `utils`'ta
ve Python `rgsx_manager.py` onu `run_server(ManagerHandler)` + SSE + tray ile sunuyor (port 5000).

**Rust handler sınıfı (api.rs, 2026-08-12):**
- *Gerçek/fonksiyonel:* `static_file`, `progress`, `history`, `queue`, `download` (torrent),
  `health`, `finalize_download_in_state`, `fallback`.
- *Placeholder (boş/statik):* `index`/`placeholder_index`, `platforms`, `search`, `translations`,
  `games`, `game_status`, `settings_get`, `system_info`, `browse_directories`, `image`, `favicon`,
  `update_cache`, `queue_post`, `queue_clear`, `queue_remove`, `settings_post`, `save_filters`,
  `clear_history`, `restart`, `support`, `shutdown`, `pause`, `resume`, `cancel` (yanıt placeholder),
  `change_password` (sadece uzunluk kontrolü), `qb_start`, `qb_password_status`.

**Davranış kuralları (göç sırasında değişmez):**
1. Her Rust route, Python karşılığıyla **birebir aynı** HTTP yanıtını dönmeli — altın referans
   `tests/test_api_contract.py` (Python) + `manager-rs/manager-http/tests/contract.rs` (Rust, 68 test).
2. SSE (`/api/events`) olay türleri (queue/history/progress/downloaded) korunur.
3. `/api/download` torrent yolu zaten Rust'te (Faz 10c/2); diğer route'lar ona paralel eklenir.
4. Kesintisiz göç: Python sunucu `RGSX_RUST_WEBUI=1` flag'iyle **köprü modunda** kapatılabilir;
   flag kapalı → mevcut Python davranışı (risk sıfır, her alt görevde korunur).

## Alt görev bölme (todo)

Her alt görev kendi `TASK-002k-N.md` dosyasında; hepsi `environment: both`, flag-gated, contract
testleriyle korunur. Sıra: keşif → katalog → durum/settings → destek/queue → qbittorrent bridge →
Web UI tek elden → göç doğrulama.

- **TASK-002k-1** Keşif + sözleşme envanteri: route→Python fonksiyonu eşlemesi; boşluk matrisi;
  çıktı `docs/roadmap/FAZ10C3_CONTRACT_MAP.md`.
- **TASK-002k-2** Katalog (platforms/search/games/image/translations) Rust'e.
- **TASK-002k-3** Durum/settings (settings_get/settings_post/save_filters/system_info/
  browse_directories/game_status) Rust'e.
- **TASK-002k-4** Destek/queue yönetimi (support/queue_*/clear_history/restart/pause/resume/
  cancel/shutdown) Rust'e.
- **TASK-002k-5** qBittorrent bridge handler'ları (change_password/qb_start/qb_password_status)
  `TorrentBackend` trait'e bağla.
- **TASK-002k-6** Web UI statik sunumu + SSE tek elden Rust; Python sunucuyu flag-gated kapat.
- **TASK-002k-7** Göç doğrulama: 364 Python + 68 Rust contract + canlı smoke; Python köprüsü kapanış.

## Doğrulama (tüm alt görevler)

- `cargo test --workspace` + `python -m pytest tests/ --noconftest` (pygame olanlar hariç) yeşil.
- `manager-http/tests/contract.rs` her eklenen route için yeni assertion içerir.
- Canlı smoke: TV UI `manager_launcher.ensure_manager` akışı `RGSX_RUST_WEBUI=1` ile kesintisiz.

## İlerleme

- 2026-08-12 — Plan araştırmayla yazıldı; 7 alt göreve bölündü (`tasks/todo/TASK-002k-*.md`).
