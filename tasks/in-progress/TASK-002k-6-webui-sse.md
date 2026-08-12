# TASK-002k-6 — Faz 10c/3/6: Web UI statik sunumu + SSE tek elden Rust

- **id:** TASK-002k-6
- **title:** `index`/`static_file` gerçek asset root + SSE tek elden; Python sunucuyu flag-gated kapat
- **status:** in-progress
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
  (98/98). `manager-bin/main.rs`: `RGSX_WEBUI_DIR` env ile statik kök override eklendi (additive).
- **BLOKE — port topolojisi kararı gerekli**: 002k-2/3/4 catalog'ı Python'a PROXY'liyor
  (Python HTTP 5000 gerekir), ama bu görev "Python HTTP sunucusunu kapat" diyor. İkisi çelişir:
  Python HTTP kapanırsa `RGSX_PYTHON_MANAGER_URL` proxy'si connection-refused olur. Kullanıcıya
  soruldu (Rust 5000 mı / 5010 mı, catalog Python'a nasıl ulaşır). Yanıt gelene dek Python skip
  değişikliği GERİ ALINDI (bozuk olurdu).
