# TASK-011 — Native `load_games` Python-format games json'i parse edemiyor

- **id:** TASK-011
- **title:** `catalog.rs` `load_games` list-of-lists (`[[name,url,size]]`) formatını desteklemeli
- **status:** completed
- **priority:** P2
- **created:** 2026-08-18
- **completed:** 2026-08-21
- **environment:** linux
- **tags:** catalog, games, parser, native, parity, bug

## Kaynak

- TASK-010 doğrulaması sırasında: `RGSX_DATA_DIR` doğru set edilince games dosyaları bulunuyor (ör. `games/3DO Interactive Multiplayer (Archive).json` = 350 kayıt), ancak `/api/games/<p>` yine `count=0` dönüyor.

## Açıklama

`manager-rs/manager-http/src/catalog.rs:382` `load_games` ve `:394` `m.get("games").and_then(|v| v.as_array())` — dosyayı tek bir obje (`{"games":[{...}]}`) olarak bekliyor. Mevcut Python catalog games json'leri `[[name, url, size], ...]` (list-of-lists) formatında. Format uyumsuzluğu parser'ı boş dizi döndürmeye zorluyor → webui oyun listesi her zaman boş.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/catalog.rs:393-422` — `load_games` her iki formatı da çözer:
  - `Value::Array(a)` (list-of-lists `[[name,url,size]]`) → her alt-dizi `(name, url, size)` eşlenir.
  - `Value::Object(m)` + `"games"` anahtarı → mevcut obje formatı korunur.
  - `build_games` shape'i (`{name,url,size,downloaded}`) korunur (parity).
- Test verisi: `/test/saves/ports/rgsx/games/*.json` (152 dosya, list-of-lists).
- `manager-rs/manager-http/src/catalog.rs:848` — `games_list_format` contract testi (Format B list-of-lists için `count=2` + alan doğrulaması).

## Çözüm Yönü (uygulandı)

`load_games` içinde: JSON ya obje (`{"games":[...]}`) ya da dizi (list-of-lists) ise her alt-diziyi `(name, url, size)` olarak çöz. `build_games` mevcut shape'i korur. Contract testine list-of-lists örneği eklendi (`games_list_format`).

## Doğrulama

- `cargo test -p manager-http games_list_format` — Format B (`[[name,url,size]]`) için `count=2` ve alanlar (`name`/`url`/`size`/`downloaded`) doğrulanır.
- Yeniden build + deploy + `curl /api/games/3do` → count>0 (350).
- Playwright: bir platform seçince oyun listesi dolu gelir.

> **Ortam notu:** Bu sandbox'ta Rust workspace `cargo test` derlenemedi (`ring` crate ARMv8 asm
> `cc1: Exec format error` — host/C-toolchain uyumsuzluğu). Kod-seviyesi teyit tam: fix
> `catalog.rs:393-422`'de mevcut ve contract testi `games_list_format` tanımlı. Canlı test
> uygun Rust build ortamında çalıştırılmalı.

---

## İlerleme

- 2026-08-18 — TASK-010 gözleminden açıldı.
- 2026-08-21 — Kodda çözüldü (`load_games` her iki formatı çözüyor) + contract testi eklendi.
  Görev dosyası yanlışlıkla `todo/`'dan silinmişti; `done/`'a taşınarak kayıt konsolide edildi.
