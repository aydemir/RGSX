# TASK-011 — Native `load_games` Python-format games json'i parse edemiyor

- **id:** TASK-011
- **title:** `catalog.rs` `load_games` list-of-lists (`[[name,url,size]]`) formatını desteklemeli
- **status:** todo
- **priority:** P2
- **created:** 2026-08-18
- **environment:** linux
- **tags:** catalog, games, parser, native, parity, bug

## Kaynak

- TASK-010 doğrulaması sırasında: `RGSX_DATA_DIR` doğru set edilince games dosyaları bulunuyor (ör. `games/3DO Interactive Multiplayer (Archive).json` = 350 kayıt), ancak `/api/games/<p>` yine `count=0` dönüyor.

## Açıklama

`manager-rs/manager-http/src/catalog.rs:382` `load_games` ve `:394` `m.get("games").and_then(|v| v.as_array())` — dosyayı tek bir obje (`{"games":[{...}]}`) olarak bekliyor. Mevcut Python catalog games json'leri `[[name, url, size], ...]` (list-of-lists) formatında. Format uyumsuzluğu parser'ı boş dizi döndürmeye zorluyor → webui oyun listesi her zaman boş.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/catalog.rs:382-415` — `load_games` / `build_games`.
- Test verisi: `/test/saves/ports/rgsx/games/*.json` (152 dosya, list-of-lists).
- `manager-rs/manager-http/tests/contract.rs:781` — games format contract testi (her iki formatı da kapsamalı).

## Çözüm yönü

`load_games` içinde: JSON ya obje (`{"games":[...]}`) ya da dizi (list-of-lists) ise her alt-diziyi `(name, url, size)` olarak çöz. `build_games` mevcut shape'i (`{name,url,size,downloaded}`) korur. Contract testine list-of-lists örneği eklenir.

## Doğrulama

- `cargo test -p manager-http` (games contract) her iki formatta geçer.
- Yeniden build + deploy + `curl /api/games/3do` → count>0 (350).
- Playwright: bir platform seçince oyun listesi dolu gelir.

---
## İlerleme

- 2026-08-18 — TASK-010 gözleminden açıldı.
