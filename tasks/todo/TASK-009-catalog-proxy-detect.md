# TASK-009 — Katalog isteklerinin hâlâ Python'a proxy'lenip proxy'lenmediğinin tespiti

- **id:** TASK-009
- **title:** Dağıtılan manager-bin'in katalog yanıtlarını saf-native verip vermediğinin tespiti
- **status:** todo
- **priority:** P2
- **created:** 2026-08-18
- **environment:** both
- **tags:** catalog, proxy, native, python-free, detection

## Kaynak

- Dil/çeviri çalışmaları sırasında, `api.rs` içinde `if !intercept_locally { if let Some(c) = &state.catalog { c.post_json(...) } }` şeklinde bir Python katalog proxy dalı olduğu not edildi. Dağıtılan ortamda `state.catalog`'un set olup olmadığı ve katalog isteklerinin native mi yoksa Python'a mı düştüğü doğrulanmadı.

## Açıklama

Dağıtılan `manager-bin` (test ortamı `/test/roms/ports/RGSX/manager-bin`) katalog uçlarını (`/api/platforms`, `/api/games`, `/api/translations`, `/api/image`) saf-native (NativeCatalog / `systems_list.json` + `games/*.json`) mı yoksa bir Python katalog örneğine mi proxy'liyor, tespit et. `RGSX_NATIVE_CATALOG` ve `state.catalog` başlatma yolunu incele.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` — katalog proxy dalı, `state.catalog` başlatma.
- `manager-rs/manager-core/src/...` — native catalog yükleme.
- Canlı uç yanıtları: `curl /api/platforms`, `/api/games/<p>`, `/api/translations?lang=`.

## Doğrulama

- `curl` ile çekilen katalog yanıtları native kaynaktan geliyor (log'da "proxy" / python referansı yok).
- `state.catalog` set değilse uçlar native dönüyor; set ise neden set olduğu ve kaldırılabileceği raporlanır.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu (dil çalışması sırasında belirlenen açık nokta).
