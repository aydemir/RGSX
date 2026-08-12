# TASK-002k-6 — Faz 10c/3/6: Web UI statik sunumu + SSE tek elden Rust

- **id:** TASK-002k-6
- **title:** `index`/`static_file` gerçek asset root + SSE tek elden; Python sunucuyu flag-gated kapat
- **status:** done
- **priority:** P2
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, faz-10c, webui, sse, göç
- **parent:** TASK-002k

## Açıklama

Rust `index`/`static_file` gerçek `static_root/index.html` + asset (CSS/JS sürüm hidratasyonu)
sunar; `/api/events` SSE tek otorite olur. Python `rgsx_web.run_server(ManagerHandler)`,
`RGSX_RUST_WEBUI=1` set edildiğinde **köprü modunda** devre dışı bırakılır (Rust 5010 yerine 5000
dinler veya tersi — kesintisiz cutover). TV UI `manager_launcher.ensure_manager` akışı her iki
modda da çalışır.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` (`index`, `static_file`, SSE)
- `manager-rs/manager-bin/src/main.rs` (port/cutover env)
- `ports/RGSX/rgsx_web/server.py` + `rgsx_manager.py` (`RGSX_RUST_WEBUI` köprüsü)
- `ports/RGSX/manager_launcher.py`

## Doğrulama

- `RGSX_RUST_WEBUI=0` (varsayılan): Python davranışı birebir. `=1`: Rust Web UI + SSE; Python
  sunucu açılmaz. Canlı smoke: Tarayıcı + TV UI her iki modda da yüklenir.

## İlerleme

- 2026-08-12 — Tanımlandı (planın parçası).
- 2026-08-12 — Rust çekirdeği ZATEN hazır: `index`/`static_file` (`static_root` + hydration +
  path-traversal koruması) ve SSE `/api/events` (`sse.rs`, native) mevcut ve contract testli
  (98/98). `manager-bin/main.rs`: `RGSX_WEBUI_DIR` env ile statik kök override.
- **Port topolojisi kararı (kullanıcı): Rust 5000, Python catalog 5001.**
  - `manager-bin/main.rs`: `RGSX_RUST_WEBUI=1` → varsayılan port 5000 (5010 yerine).
  - `rgsx_manager.py`: `RGSX_RUST_WEBUI=1` → Python SADECE `RGSX_CATALOG_PORT` (vars. 5001)
    üzerinden catalog servis eder (`run_server` portu 5001); `RGSX_PYTHON_MANAGER_URL`=:5001,
    `RGSX_MANAGER_BIN_PORT`=5000, `RGSX_RUST_DAEMON`=1 set edilir (Rust 5000'i alır).
  - TV UI portu (5000) DEĞİŞMEZ → kesintisiz cutover; launcher değişikliği GEREKMİYOR
    (ensure_manager 5000'de Rust'ı sağlık kontrol eder).
  - Varsayılan (flag kapalı) davranış birebir korunur.
