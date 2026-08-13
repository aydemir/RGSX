# TASK-002l — Faz 10c: librqbit uçtan uca (HTTP katmanı) indirme doğrulaması

- **id:** TASK-002l
- **title:** manager-bin HTTP akışında librqbit `download_torrent`'in gerçek indirme yapması (proxy bypass + e2e test)
- **status:** done
- **priority:** P1
- **created:** 2026-08-13
- **environment:** both
- **tags:** rust, librqbit, faz-10c, torrent, e2e

## Kaynak

- `manager-http/src/api.rs:352-356` — `POST /api/download`: `state.catalog` varsa **her** istek Python'a proxy edilip erken dönüyor.
- `manager-torrent/src/lib.rs:251` — `LibrqbitEngine::download_torrent` (tam akış: add→wait→resolve→link_or_copy) — kanıtlanmış çalışır (`examples/live_torrent.rs` engine-seviyesi Sintel indirir).
- `manager-rs/manager-bin/src/main.rs:222-225` — `RGSX_PYTHON_MANAGER_URL` set ise `catalog = Some` → canlı RetroBat kurulumunda **catalog daima mevcut**.
- Önceki analiz (oturum 2026-08-13): canlıda `catalog` mevcut olduğundan `/api/download` Python'a proxy → librqbit indirme yolu hiç gerçek trafik görmüyor.

## Kök Neden

`download` handler'ında `state.catalog.is_some()` kontrolü, **katalog verisi çözümü** (game_index→url) ile **indirme motoru seçimi** (librqbit vs python) kavramlarını birbirine bağlamış. Oysa bunlar ortogonal:

- Katalog/UI verisi → Python'dan gelir (`RGSX_PYTHON_MANAGER_URL`, strangler pattern).
- İndirme motoru → `RGSX_TORRENT_ENGINE` env'iyle seçilir (varsayılan `librqbit`).

Canlı kurulumda `catalog` daima `Some` olduğundan, `api.rs:352` tüm `/api/download` isteklerini Python'a proxy ediyor ve `bridge.download_torrent` (librqbit) **asla çağrılmıyor**. Engine kendi başına sağlam (`live_torrent.rs` ile kanıtlı) ama manager HTTP akışında bypass edilmiş durumda. Yani "librqbit indirme çalışıyor mu?" sorusunun cevabı **katman seviyesinde bilinmiyor**.

## Davranış Kuralları (uygulanacak)

1. `RGSX_TORRENT_ENGINE=librqbit` (varsayılan) ve istek **doğrudan çözülmüş** bir `url` taşıyorsa (`body.url` mevcut), indirme **librqbit'e** yönlenir — `catalog` var olsa bile proxy EDİLMEZ.
2. `game_index`/`game_name` ile gelen (henüz çözülmemiş) isteklerde katalog çözümü **Python'a** proxy edilir (mevcut davranış korunur) — çünkü URL çözümü Python'da.
3. `RGSX_TORRENT_ENGINE=python` ise her şey mevcut gibi Python'a proxy edilir (fallback korunur).
4. Doğrulama: gerçek bir torrent (Sintel) `manager-http` üzerinden librqbit ile indirilip `dest_path`'e sonlanmalı; offline contract testi handler'ın engine'i çağırdığını kanıtlamalı.

## Kapsam / Dosyalar

- `manager-http/src/api.rs` — `download()` routing: `url`+librqbit → engine; game_index → proxy.
- `manager-http/tests/live_download.rs` (yeni) — gerçek e2e (env-gated `RGSX_LIVE_TORRENT_TEST=1`) + offline contract testi (FakeEngine çağrımı kaydeder).
- `docs/architecture/RUST_MIGRATION_NOTES.md` — bölüm ek: "librqbit download bypass kök nedeni + çözüm".

## Doğrulama

- Offline: `cargo test -p manager-http --test live_download` — FakeEngine `download_torrent` çağrıldı mı (ve catalog proxy'ye DÜŞMEDİ mı) assert edilir.
- Canlı/CI: `RGSX_LIVE_TORRENT_TEST=1 cargo test -p manager-http --test live_download -- --ignored` → Sintel torrent'i librqbit ile iner, `dest_path` oluşur.
- `cargo test -p manager-torrent` (mevcut engine testleri) yeşil kalır.

---

## İlerleme

- 2026-08-13 — kök neden + davranış kuralları yazıldı; api.rs routing kararı kullanıcı onayına sunuldu (yalnızca torrent şemaları: magnet:/rgsx+torrent:/.torrent).
- 2026-08-13 — `api.rs` routing düzeltildi (`is_torrent_url` + intercept guard); catalog+bridge senaryosunda torrent URL'i artık Python'a proxy EDİLMİYOR, engine'e yönleniyor. Düz http URL'ler proxy'ye devam.
- 2026-08-13 — `contract.rs`'e 2 test eklendi (torrent intercept + non-torrent proxy); `live_download.rs` (`#[ignore]`, gerçek Sintel) eklendi; `CARGO_TARGET_DIR` override ile 101 contract testi yeşil.
