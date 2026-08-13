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

## 0. Stratejik karar (en kritik nokta)

**TVUI bir web arayüzü DEĞİL.** `ports/RGSX/tvui.py` + `display/*` + `controls/*` saf
**pygame** ile çizilen native bir 10-foot UI'dır (TV/monitörde tam ekran). Bunu birebir
Rust'a pygame benzeri bir grafik kütüphanesiyle (macroquad/egui/bevy) port etmek en büyük
ve en düşük ROI'li iş olurdu.

**Öneri: İki UI'ı TEK bir web frontend'te birleştir.**
- Aynı **Vue 3 SPA**, iki "shell" ile çalışır:
  - **WebUI shell:** tarayıcıda (mevcut `RGSX_RUST_WEBUI` akışı).
  - **TVUI shell:** SPA tam ekran kiosk modunda (`?mode=tv`), gamepad/IR remote JS
    Gamepad API + tuş olaylarıyla; Rust binary'si bunu ya (a) kiosk Chromium ya da
    (b) `wry`+`tao` webview ile native pencerede gösterir.
- Böylece `display/*` (30+ dosya) ve `controls/*` emektar pygame kodu **retire** edilir,
  tek bir SPA bakımı kalır. Bu, geçiş maliyetini yarıdan fazla düşürür.

Alternatif (önerilmez): pygame→Rust native render. Sadece offline/webkit-yasaklı
ortamlar için düşünülür; ayrı bir alt faz olarak not edilir (Faz 12b-alt).

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
| `wry` + `tao` | TVUI native webview shell (bkz. §0) | 12b |
| `igd` / `miniupnpc` | UPnP port mapping (`network/upnp.py` yerine) | 12e |
| `reqwest` (mevcut) | TheGamesDB + debrid/1Fichier istemcisi | 12c |

### Frontend (YENİ, `webui/`)
- **Vue 3 + Vite** (önerilen; qBittorrent mirasıyla uyumlu, hafif, SSE kolay).
  Alternatif: Svelte (daha az runtime). **Leptos/Yew (Rust→WASM) önerilmez** —
  SSE/Gamepad entegrasyonu saf JS'te daha olgun.
- Canlı akış: `EventSource('/api/events')` → reaktif store → progress bar otomatik render.
  (Backend SSE `manager-http/src/sse.rs` zaten var — TASK-002m.)
- TV modu: `@media` kiosk CSS + Gamepad API + uzak kumanda tuş eşlemesi.

### Build/paketleme
- `webui/` Vite build → `manager-http` tarafından `ServeDir` ile sunulur.
- `RGSX_WEBUI_DIR` (zaten var) statik kök override eder.
- Windows cross-compile: mevcut `rust-toolchain.toml` + `cfg(windows)` deseni korunur;
  `wry` Linux'ta webkit2gtk, Windows'ta webview2 gerektirir (build runner not edilecek).

---

## 2. Faz planı (strangler: önce proxy, sonra native, flag-gated cutover)

### Faz 12a — WebUI native SPA
- `webui/` scaffold (Vue 3 + Vite). Mevcut WebUI endpoint sözleşmeleri birebir korunur
  (Faz 10c/3 contract testleri: 102 contract + SSE yeşil).
- `tower-http::ServeDir` ile sunum; SSE progress barları canlı akar (refresh yok).
- `RGSX_RUST_WEBUI=1` → Rust placeholder yerine gerçek SPA.
- Test: contract testleri yeşil kalır; SPA build CI'da çalışır.

### Faz 12b — TVUI shell (webview)
- Yeni `manager-tvui` crate: `wry`+`tao` ile aynı SPA'yı tam ekran kiosk render eder.
  Gamepad/IR remote → JS olayları. `?mode=tv` ile SPA TV layout'u.
- `ports/RGSX/display/*` ve `controls/*` **retire** edilir (sözleşme: aynı endpoint'ler).
- Fallback: webview binary yoksa kiosk Chromium spawn (Windows Retrobat senaryosu).
- Test: TV modu contract (aynı JSON), gamepad eşleme birim testi.

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
- Settings şeması (`rgsx_settings.py`/`config.py`) → Rust şema + validasyon (sözleşme).

---

## 3. Aklına gelmeyen ama geçişte ZORUNLU modüller (kör nokta listesi)

1. **TVUI pygame native'dir, web DEĞİL** — en büyük gizli maliyet. §0 birleştirme önerisi.
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
   webview senaryosunda JS Gamepad API + config şemasına dönüşür.
9. **Accessibility** (`accessibility.py`) — yüksek kontrast, font ölçekleme; yeni UI'de korunmalı.
10. **Settings kalıcılığı** (`rgsx_settings.py`, `config.py`) — yalnız okuma/yazma değil,
    şema + validasyon sözleşmesi port edilmeli.
11. **UPnP** (`network/upnp.py`) — router port mapping; `igd` krate (Faz 12e).
12. **Virtual keyboard / folder browser** (`display/virtual_keyboard.py`,
    `display/folder_browser.py`) — SPA bileşenlerine dönüşür.
13. **Embedded cache** (`build_embedded_caches.py`) — Python'ın bellek/embed cache'i;
    Rust eşdeğer cache geçersizleme mantığı gerektirir.
14. **Cross-platform build** — Windows-only tray/firewall zaten `cfg(windows)`; webview
    da Windows/Linux/macOS farklı native dep gerektirir (webkit2gtk vs webview2).
15. **Gamepad/IR remote input katmanı** — UI değil, girdi cihazı katmanı; webview'te
    Gamepad API, native'de `gilrs` (Rust gamepad) ile çözülür.
16. **Proxy contract birebir korunmalı** — geçiş boyunca `FakeCatalog`/`FakeProgressEngine`
    benzeri contract testleri (102 contract + 9 engine) her fazda yeşil kalmalı.

---

## 4. Risk ve sıralama
- Düşük riskten yükseğe: 12a (SPA, zaten SSE var) → 12c (catalog native) →
  12d (HDD scan) → 12b (TVUI webview) → 12e (download/çevre).
- Her faz flag-gated; Python fallback korunur (Faz 10c deseni).
- TVUI birleştirme kararı (§0) onaylanmadan 12b'ye başlanmamalı.
