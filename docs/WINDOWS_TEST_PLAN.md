# Windows Test Plan — Tamamlanan Rust Görevleri

> Amaç: Linux sandbox'ta doğrulanamayan, **yalnız gerçek Windows makinede** (veya
> `x86_64-pc-windows-gnu` cross-compile + Windows çalıştırma) teyit edilecek kalemleri
> kaydetmek. Tamamlanan görevler `tasks/done/`'a taşındıktan sonra bu not, "Windows'da
> ne test edeceğim" sorusunu kapatır.
>
> İlgili skill: `rgsx-windows-build`. Linux build notu: `rgsx-rust-build`.

## 0. Genel Windows doğrulama matrisi

| Adım | Komut (Windows makine) | Beklenen |
|---|---|---|
| Cross-compile / release | `cd manager-rs && cargo build --release -p manager-bin` | Hatasız; `target/release/manager-bin.exe` |
| Windows-only crate | `cargo build --release -p manager-windows` | `cfg(windows)` dalı linklenir (tray/autostart/firewall) |
| Birim + contract testleri | `cargo test` (workspace) | Linux ile aynı yeşil sayı (103 contract + core) — OS-bağımsız router testleri |
| Native ayar persist | `RGSX_NATIVE_SETTINGS=1 RGSX_SETTINGS_PATH=%APPDATA%/rgsx/rgsx_settings.json manager-bin.exe` | `rgsx_settings.json` Windows yollarıyla yazılır; `language` yoksa enjekte edilmez |

> Not: `manager-rs/.cargo/config.toml` Windows `target-dir = "C:/Users/lv/RGSX/rust-target"`
> yalnız Windows'ta geçerlidir; Linux sandbox'ta `CARGO_TARGET_DIR` override kullanılır.

## 1. Göreve göre Windows test kalemleri

### Faz 10c — Rust daemon + torrent (TASK-002i, 002j, 002l, 002m, 002k-7)
- **Daemon (002i):** `manager-bin.exe` arka planda başlar; `GET /api/health` → `{success, manager}`; watchdog state machine `Init→Running`. Windows Service olarak mı, konsol mu? Launcher davranışı Windows'da ayrı (Tray yoksa konsol penceresi).
- **Torrent (002j/002l/002m):** `librqbit` engine `RGSX_TORRENT_ENGINE=librqbit` ile canlı indirme; SSE `progress` olayı Windows'ta da akar. **Gerçek Windows test:** bir torrent indir, `%` ve hız akışını UI'da doğrula (sandbox'ta canlı torrent testi yapılamıyor).
- **Doğrulama (002k-7):** contract 102 (şimdi 103) + canlı smoke Windows'da tekrar koşulur.

### Faz 12a — WebUI SPA (TASK-002n)
- `webui/dist/` Windows'ta `RGSX_RUST_WEBUI=1` ile sunulur; `ServeDir` traversal koruması Windows yolları (`\`) ile doğrulanır.
- SPA route'ları (`/settings`, `/downloads`) hydrate edilmiş `index.html` döndürür.

### Faz 12b — TVUI (TASK-002q)  ← yön (B): native SDL2
- `RGSX_TVUI=1` → `manager-tvui` `rust-sdl2` ile tam ekran 10-foot native render (SPA/webview yok). `theme.json` `serde_json` ile yüklenir. `wry`+`tao` webview bağımlılığı **yok**.
- Gamepad/ok tuş navigasyonu `native_input.rs` (gilrs) ile; gerçek gamepad Windows'ta doğrulanır.

### Faz 12c — Catalog native (TASK-002o)
- `RGSX_NATIVE_CATALOG=1` ile `systems_list.json`/`games/<platform>.json`/`images/` Windows `RGSX_DATA_DIR` altından okunur; yol ayraçları (`\`) ve `RGSX_ENTRY_WINDOWS` platform eşlemesi doğrulanır.

### Faz 12d — HDD scan (TASK-002p) — **Windows'a özgü en kritik**
- `manager-scan/src/gamelist.rs` **Windows varyantı** mevcut gamelist entry'lerini KORUR ve RGSX entry'sini merge eder (`write_gamelist(root, merge=true)`); Linux yalnız RGSX entry yazar.
- **Windows test:** gerçek bir `gamelist.xml` (EmulationStation) üzerinde scan çalıştır → mevcut oyunlar silinmez, RGSX node'u eklenir. Bu Linux'ta test EDİLEMEZ.

### Faz 12e — Download manager (TASK-002r)
- `RGSX_NATIVE_DOWNLOAD=1` ile DDL `DirectResolver` → `reqwest` indirme; Windows'ta `RGSX_DOWNLOADS_FOLDER` (ters-slash) doğru çözülür.
- `OneFichierResolver`/`RealDebridResolver` kimlik gerektirir; Windows'da credential ile smoke.

### Faz 12f — Settings (TASK-002s)
- `RGSX_NATIVE_SETTINGS=1` ile `Settings::save()` → `RGSX_SETTINGS_PATH` (Windows: `%APPDATA%/rgsx/`). `validate()` invariantları; `language` `Option` round-trip.
- **Henüz native DEĞİL (sonraki faz):** `auto_extract`, `api_keys` (ayrı `.txt`), `web_service_at_boot`/`custom_dns_at_boot` (Windows'ta systemd yok — bu alanlar Windows'da anlamsız/atlanır). Windows testinde bu alanların native save'e sızmadığı (strip) doğrulanır.

## 2. Windows-only crate: manager-windows (cfg(windows))
- **Tray:** systray ikonu + menü (aç/kapat/quit) gerçek Windows'ta görünür.
- **Autostart:** kayıt defteri (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`) yazımı.
- **Firewall:** `windows/scripts/rgsx_firewall_setup.ps1` PowerShell kuralı (port 5000/5010) uygulanır.
- NSIS installer (`windows/` altı) Windows'da derlenir; bu ortamda YALNIZ düzenlenir.

## 3. Bilinen engeller (bu ortamda test EDİLEMEZ)
- `\` yol ayraçları, registry, Windows Defender/Firewall — yalnız statik doğrulama.
- `manager-tvui` SDL2 native (webview2 feature yok).
- Canlı torrent/DDL indirme — sandbox ağ sınırı.

---
*Kaynak: tamamlanan görevler `tasks/done/TASK-002i..m`, `TASK-002n..s`; skill `rgsx-windows-build`.*
