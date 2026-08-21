# ROADMAP — Faz 12: WebUI + TVUI Rust Geçişi

> Bağlam: Faz 10b/10c zaten Rust'e torrent motoru (librqbit) + katalog/state/queue/destek
> route'larını **strangler/proxy** yöntemiyle taşıdı. Rust `manager-http` şu an katalog ve
> diğer route'ları `127.0.0.1:5001` (Python) üzerinden proxy'liyor; native mantık ayrı
> alt fazlara ertelendi. Bu roadmap o native portları + UI katmanını kapar.
>
> Kodla doğrulandı (codegraph): `manager-rs/manager-http/src/catalog.rs` (`CatalogSource`
> trait + `PythonCatalog`), `ports/RGSX/rgsx_web/*` (WebUI), `ports/RGSX/tvui.py` +
> `ports/RGSX/display/*` (TVUI, pygame native), `ports/RGSX/update_gamelist*.py`
> (gamelist.xml HDD scan çıktısı), `ports/RGSX/network/*` (download worker),
> `ports/RGSX/scraper.py` (TheGamesDB katalog).

---

## 0. Stratejik karar (en kritik nokta) — GÜNCEL: native SDL2 + `.json` tema yönü SEÇİLDİ

**TVUI bir web arayüzü DEĞİL.** `ports/RGSX/tvui.py` + `display/*` + `controls/*` saf
**pygame** ile çizilen native bir 10-foot UI'dır (TV/monitörde tam ekran). Bu, EmulationStation
tarzı bir UI'dır (platform/oyun ızgarası, menüler, sanal klavye) ve ES'in XML tema
yaklaşımından esinlenir — ANCAK temaları **`.json`** olarak portluyoruz (XML ayrıştırma yok;
`serde_json` ile tip-güvenli yükleme).

**SEÇİLEN YÖN (B): Native Rust + SDL2, EmulationStation-tarzı, `.json` tema.**
- **Render:** `rust-sdl2` ile tam ekran 10-foot çizim; pygame `draw_*` fonksiyonları birebir
  SDL2 primitives'e portlanır (rect / text+font / image+box-art / transition).
- **Tema:** `display/colors.py` + `fonts.py` + `transitions.py` + `icons.py` → **`theme.json`**
  şeması (renk paleti + arka plan preset'leri + font ailesi + geçiş efektleri + ikon seti).
  Kullanıcı `.json` tema dosyasıyla görünümü serbestçe değiştirir (ES `theme.xml` yerine).
- **Girdi:** `manager-tvui/src/native_input.rs` (gilrs/SDL gamepad) ZATEN portlu (TASK-005) —
  SDL2 yolunda doğrudan kullanılır; webview'in JS Gamepad API'sine gerek kalmaz.
- `display/*` (30+ dosya) + `controls/*` emektar pygame kodu, port tamamlanınca **retire**
  edilir; yerine `manager-tvui` (SDL2) + `.json` tema altyapısı geçer.
- Davranış parity'si ZORUNLU (bkz. `FAZ12_PARITY_STRATEGY.md`); yapı parity'si serbest.

**Superseded alternatif:** Vue 3 SPA + `?mode=tv` webview (`wry`+`tao`). Webkit2gtk/webview2
native bağımlılık yükü ve TV/RetroBat ortamlarında webview güvenilirliği nedeniyle tercih
edilmedi. SPA/webview TVUI görevleri (**TASK-012a..f**) **superseded** olarak işaretlendi;
yerine native SDL2 görevleri (**TASK-012g..l**) geçti. WebUI (tarayıcı SPA) hattı etkilenmez.

---

## 0.5 Parity stratejisi (kontrollü ayrılma)

Geçiş sırasında Python'dan **ne zaman ve nasıl** bilerek ayrılacağımız:
[`FAZ12_PARITY_STRATEGY.md`](FAZ12_PARITY_STRATEGY.md). Özet: davranış parity'si
zorunlu, yapı parity'si serbest; ayrılma Q1/Q2/Q3 kriteriyle ve flag-gated geçişle
yapılır, her ayrılma divergence-note ile belgelenir.

---

## 1. Teknoloji yığını + bağımlılıklar

### Backend (mevcut workspace'e eklenir)
Mevcut: `tokio`, `axum 0.7`, `serde`, `serde_json`, `reqwest` (rustls-tls),
`percent-encoding`, `async-trait`, `futures-util`, `tracing`, `librqbit`.

Yeni eklenmesi gerekenler:
| Krate | Görevi | Faz |
|---|---|---|
| `tower-http` | `ServeDir` (statik SPA), CORS, gzip, timeout | 12a |
| `quick-xml` / `roxmltree` | `gamelist.xml` okuma/yazma (HDD scan çıktısı) | 12d |
| `walkdir` | ROMS_FOLDER özyinelemeli tarama | 12d |
| `sysinfo` | disk kullanımı (`get_disk_usage`) | 12d |
| `notify` | dosya sistemi watch → canlı rescan (opsiyonel) | 12d |
| `image` (rust) | box-art indirme + thumbnail (`PIL` yerine) | 12c |
| `dirs` / `directories` | platform cache/config yolları | 12c/12d |
| `sha2`+`base64` | image/catalog cache anahtarı | 12c |
| `fluent` / `i18n-embed` | dil/çeviri (`language.py` yerine) | 12e |
| `sdl2` (rust-sdl2) | TVUI native SDL2 render shell + `.json` tema yükleyici (bkz. §0) | 12g |
| `igd` / `miniupnpc` | UPnP port mapping (`network/upnp.py` yerine) | 12e |
| `reqwest` (mevcut) | TheGamesDB + debrid/1Fichier istemcisi | 12c |

### Frontend (YENİ, `webui/`)
- **Vue 3 + Vite** (önerilen; qBittorrent mirasıyla uyumlu, hafif, SSE kolay).
  Alternatif: Svelte (daha az runtime). **Leptos/Yew (Rust→WASM) önerilmez** —
  SSE/Gamepad entegrasyonu saf JS'te daha olgun.
- Canlı akış: `EventSource('/api/events')` → reaktif store → progress bar otomatik render.
  (Backend SSE `manager-http/src/sse.rs` zaten var — TASK-002m.)
- (Not: TVUI artık native SDL2 — bkz. §0; WebUI SPA yalnız tarayıcı modunda kalır, TV/Gamepad
  katmanı `manager-tvui`'dedir, webview/`wry` bağımlılığı yok.)

### Build/paketleme
- `webui/` Vite build → `manager-http` tarafından `ServeDir` ile sunulur.
- `RGSX_WEBUI_DIR` (zaten var) statik kök override eder.
- Windows cross-compile: mevcut `rust-toolchain.toml` + `cfg(windows)` deseni korunur;
  TVUI native SDL2 olduğundan `wry`/`webview2` bağımlılığı yok; SDL2 cross-compile
  (Windows'ta `SDL2.dll`, Linux'ta `libSDL2`) build runner not edilecek.

---

## 2. Faz planı (strangler: önce proxy, sonra native, flag-gated cutover)

### Faz 12a — WebUI native SPA
- `webui/` scaffold (Vue 3 + Vite). Mevcut WebUI endpoint sözleşmeleri birebir korunur
  (Faz 10c/3 contract testleri: 102 contract + SSE yeşil).
- `tower-http::ServeDir` ile sunum; SSE progress barları canlı akar (refresh yok).
- `RGSX_RUST_WEBUI=1` → Rust placeholder yerine gerçek SPA.
- Test: contract testleri yeşil kalır; SPA build CI'da çalışır.

### Faz 12b — TVUI shell (native SDL2)  ← bkz. §0, yön (B)
- Yeni `manager-tvui` crate: `rust-sdl2` ile `display/*`+`controls/*` pygame `draw_*`'larını
  birebir SDL2 primitives'e portlar (rect/text+font/image+box-art/transition). `?mode=tv`
  SPA yok — native 10-foot render. `theme.json` (`colors.py`+`fonts.py`+`transitions.py`+
  `icons.py`) `serde_json` ile yüklenir (ES `theme.xml` yerine).
- `ports/RGSX/display/*` ve `controls/*` port tamamlanınca **retire** edilir (TASK-012l).
- Girdi: `native_input.rs` (gilrs) zaten portlu (TASK-005); webview JS Gamepad API'sine gerek yok.
- Test: 102 contract (loading/platform_grid/game_list/progress) + SSE yeşil; `RGSX_TVUI=0`
  → Python fallback korunur, `RGSX_TVUI=1` → SDL2 native.

### Faz 12c — Catalog native port (Python→Rust)  ← "catalog indirme"
Mevcut `CatalogSource` trait (`catalog.rs`) `PythonCatalog`'tan `NativeCatalog`'a döner.
- **Yeni crate `manager-catalog`:**
  - TheGamesDB v1 istemcisi (`reqwest`, API key `TheGamesDBAPI.txt`'ten).
  - `systems_list.json` loader (Python `scraper.PLATFORM_MAPPING` + `config`).
  - search/games/translations/image endpoint'leri native.
  - **Debrid/1Fichier resolver'ları** (`network/one_fichier.py`, `utils/torrent.py`):
    token yenileme + link üretme → Rust'e. Bu "catalog indirme" linklerinin kaynağı.
  - Box-art indirme + disk cache (`image` + `sha2`/`base64`).
  - `CatalogSource` trait'ini native implemente eder; `AppState.catalog` swap edilir.
- Cutover: `RGSX_NATIVE_CATALOG=1` → Python catalog portu 5001 devre dışı.
- Test: `FakeCatalog` benzeri native test; Python yanıtıyla diff contract testi.

### Faz 12d — HDD scan native port (Python→Rust)  ← "hdd tarama"
- **Yeni crate `manager-scan`:**
  - `walkdir` ile `ROMS_FOLDER` tarama → platform klasörüne göre gruplama.
  - `quick-xml` ile `gamelist.xml` oku/birleştir/yaz —
    **Linux + Windows varyantları ayrı** (`update_gamelist.py` vs `update_gamelist_windows.py`,
    path/fields farkı var, birebir port).
  - History eşleme (`history_matches.py` moved_paths çözümü) native.
  - `/api/games` scan endpoint + SSE ile canlı scan progress.
  - Opsiyonel `notify` ile canlı rescan.
- Deps: `walkdir`, `quick-xml`, `sysinfo`, `notify`.
- Test: `gamelist.xml` round-trip (Python üretilenle byte-diff).

### Faz 12e — Download manager + çevresel modüller native
- HTTP direct download worker (`network/queue.py`, `http_download.py`, `download_state.py`)
  → `manager-download` crate (`reqwest` + resume/ranged + retry backoff).
  `DownloadState`/`DownloadEvent` enum'ları `manager-core`'a taşınır.
- UPnP (`network/upnp.py`) → `igd` krate.
- i18n (`language.py`) → `fluent`.
- OTA update (`check_for_updates`/`apply_pending_update`) → native veya Python'da tut.
- Settings şeması (`rgsx_settings.py`/`config.py`) → Rust şema + validasyon (sözleşme). **TAMAM (Faz 12f, TASK-002s):** `manager-core/src/settings.rs` typed `Settings` + `load/save/validate`; `RGSX_NATIVE_SETTINGS=1`. Yan-etkili alanlar (auto_extract/api_keys/linux-toggle) sonraki faza bırakıldı (Option A).

---

## 3. Aklına gelmeyen ama geçişte ZORUNLU modüller (kör nokta listesi)

1. **TVUI pygame native'dir, web DEĞİL** — SEÇİLEN yön native SDL2 + `.json` tema (bkz. §0). SPA/webview alternatifi superseded.
2. **gamelist.xml (EmulationStation) üretimi** — `update_gamelist.py` (Linux) +
   `update_gamelist_windows.py` (Windows) ayrı mantık; ikisi de port edilmeli (Faz 12d).
3. **History matching** (`history_matches.py`) — moved_paths çözümü; sessiz bağımlılık.
4. **Box-art image cache/thumbnail** (`PIL`/`pygame.image`) → Rust `image` krate (Faz 12c).
5. **Debrid/1Fichier resolver'ları** (`one_fichier.py`, `utils/torrent.py`) — auth + token
   yenileme + link üretme. "Catalog indirme" bunlardan beslenir (Faz 12c).
6. **OTA self-update** (`check_for_updates`, `apply_pending_update`, `OTA_data_ZIP`) —
   otomatik güncelleme; port veya Python'da tutulmalı.
7. **i18n/dil algılama** (`language.py`, TASK-003-faz11) → `fluent` (Faz 12e).
8. **Controls mapper** (`controls_mapper.py`, `controls/*`) — gamepad/keyboard eşleme;
    native SDL2'de `native_input.rs` (gilrs) gamepad eşlemesi doğrudan kullanılır.
9. **Accessibility** (`accessibility.py`) — yüksek kontrast, font ölçekleme; yeni UI'de korunmalı.
10. **Settings kalıcılığı** (`rgsx_settings.py`, `config.py`) — yalnız okuma/yazma değil,
    şema + validasyon sözleşmesi port edilmeli.
11. **UPnP** (`network/upnp.py`) — router port mapping; `igd` krate (Faz 12e).
12. **Virtual keyboard / folder browser** (`display/virtual_keyboard.py`,
     `display/folder_browser.py`) — SDL2 native bileşenlerine (virtual_keyboard.rs/folder_browser.rs) dönüşür.
13. **Embedded cache** (`build_embedded_caches.py`) — Python'ın bellek/embed cache'i;
    Rust eşdeğer cache geçersizleme mantığı gerektirir.
14. **Cross-platform build** — Windows-only tray/firewall zaten `cfg(windows)`; webview
     da Windows/Linux farklı native dep gerektirir (SDL2.dll / libSDL2); proot/CI'de SDL2 bulunmayabilir (build runner not edilecek).
15. **Gamepad/IR remote input katmanı** — UI değil, girdi cihazı katmanı; webview'te
     native SDL2'de `gilrs` (Rust gamepad) ile çözülür.
16. **Proxy contract birebir korunmalı** — geçiş boyunca `FakeCatalog`/`FakeProgressEngine`
    benzeri contract testleri (102 contract + 9 engine) her fazda yeşil kalmalı.

---

## 4. Risk ve sıralama
- Düşük riskten yükseğe: 12c (catalog native) → 12d (HDD scan) →
  12g (TVUI SDL2 shell + `.json` tema) → 12h (çekirdek ekranlar) → 12i (menüler) →
  12j (klavye/folder) → 12k (erişilebilirlik) → 12l (cutover). 12e (download/çevre) bağımsız;
  12a/12b SPA/webview superseded.
- Her faz flag-gated; Python fallback korunur (Faz 10c deseni).
- TVUI birleştirme kararı (§0) onaylanmadan 12b'ye başlanmamalı.
