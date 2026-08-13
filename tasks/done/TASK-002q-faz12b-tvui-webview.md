# TASK-002q — Faz 12b: TVUI shell (WebUI SPA + kiosk/webview)

> Bağımlı: ROADMAP_FAZ12_RUST_WEBUI_TVUI.md (onaylandı). Strateji: pygame TVUI native
> port YERİNE, mevcut WebUI Vue 3 SPA'sı `?mode=tv` (10-foot layout + gamepad/kumanda
> nav) ile native pencerede gösterilir; tek frontend bakımı kalır. `display/*` retire.

## Uygulama
- `webui/src/App.vue`: `?mode=tv` algılama → `.app.tv` büyük layout (vh ölçekli).
  Ok tuşları + `navigator.getGamepads()` (axis/button 12-13) ile seçim gezinmesi.
- `manager-rs/manager-tvui` (yeni crate): `launch(port)` → `http://127.0.0.1:<port>/?mode=tv`
  kiosk tarayıcı (chromium/chrome) spawn. `RGSX_TVUI=1` ise `manager-bin` ayrı thread'de çağırır.
- `manager-bin/src/main.rs`: `RGSX_TVUI=1` → TVUI thread (sunucuyu bloklamaz; headless'ta
  uyarı loglanır).

## Doğrulama
- `npm run build` (webui) → dist üretir. `cargo build -p manager-tvui` + `-p manager-bin` yeşil.
- Runtime: `manager-tvui 5000` → tarayıcı yoksa graceful "kiosk tarayıcı bulunamadı" (exit 1).
  `manager-bin RGSX_TVUI=1` → sunucu 200, TVUI uyarısı loglanır, etkilenmez.

## Bilinen sapma / ertelenen
- Native webview embedding (`wry`+`tao`) BU ortamda tao'nun `gdk-3` `links` çakışması
  nedeniyle workspace lockfile'ı çözümlenemediği için şimdilik DEVRE DIŞI. Varsayılan yol
  harici kiosk tarayıcıdır. `webview` feature (wry penceresi) uygun makinede (webkit2gtk/
  webview2) ayrı alt görev olarak eklenecek.
