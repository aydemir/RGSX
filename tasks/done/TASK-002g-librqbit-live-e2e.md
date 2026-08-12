# TASK-002g — Faz 10b: librqbit canlı uçtan uca doğrulama + varsayılan kararı

- **id:** TASK-002g
- **title:** librqbit embedded engine'i canlı uçtan uca doğrula (manager-bin + HTTP /api/download)
- **status:** done
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, librqbit, manager-bin, integration, faz-10b
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10b (`librqbit` adayı).
- TASK-002f (librqbit engine çekirdek + indirme akışı + Rust-HTTP /api/download endpoint'i) ✅.
- Kullanıcı onayı (2026-08-12): "önerini onaylıyorum" → canlı uçtan uca doğrulama + varsayılan
  kararı.

## Açıklama

`RGSX_TORRENT_ENGINE=librqbit` set edildiğinde `manager-bin` `LibrqbitEngine`'i in-process
kurar ve `AppState.bridge`'e verir. Hedef: gerçek bir torrent ile tam akışı canlı doğrulamak:

1. `manager-bin` binary'si `RGSX_TORRENT_ENGINE=librqbit` + `RGSX_DOWNLOADS_FOLDER` ile ayağa kalkar.
2. `POST /api/download` (url=public .torrent, game_name, platform) yapılır.
3. Arka plan `bridge.download_torrent` → `finalize_download_in_state` → history `Download_OK`
   + `downloaded[platform]` + dosya `downloads_folder`'a çıkar.
4. `GET /api/queue` / `/api/events` (SSE) ile sonuç izlenir.

Doğrulama başarılıysa **varsayılan motor** kararı verilir (şu an Python bridge öntanımlı).
Varsayılan değişikliği davranışı etkilediğinden kullanıcı onayıyla uygulanır.

## Kapsam / Dosyalar

- `manager-rs/manager-bin/src/main.rs` — `resolve_engine` (librqbit dalı)
- `manager-rs/manager-http/src/api.rs` — `download` + `finalize_download_in_state`
- `manager-rs/manager-torrent/src/lib.rs` — `LibrqbitEngine::download_torrent`

## Doğrulama

- `manager-bin` `librqbit` modunda başlar (log: "torrent engine: librqbit (embedded)").
- `/api/download` sonrası `downloads_folder`'da gerçek dosya belirir.
- `GET /api/queue` (veya history) `Download_OK` gösterir.
- Varsayılan kararı kullanıcıya sunulur.

## İlerleme

- 2026-08-12 — Tanımlandı (kullanıcı onayı).
- 2026-08-12 — **Canlı uçtan uca doğrulama ✅** (aarch64 Linux): `manager-bin` `RGSX_TORRENT_ENGINE=librqbit`
  ile başlatıldı (log "torrent engine: librqbit (embedded)"); `POST /api/download`
  (url=https://webtorrent.io/torrents/sintel.torrent, game_name=Sintel, platform=Film) →
  arka plan `bridge.download_torrent` → `finalize_download_in_state` → history `Download_OK`
  (progress 100), `downloads/Sintel/Sintel.mp4` (129MB) indi, `dest_path` (`downloads/sintel.torrent`)
  hard-link ile oluştu (inode paylaşımlı, `link_or_copy` teyit). `downloaded["Film"]` güncellendi.
  Sonuç: tam Rust yığını (manager-bin → AppState.bridge=LibrqbitEngine → HTTP → finalize) gerçek
  torrent ile çalışıyor.
- **AÇIK KARAR → KAPANDI (2026-08-12):** Kullanıcı kararı: **Python bridge varsayılan kalır,
  librqbit opt-in + belgelenir**; "**Windows'ta derlendiğinde varsayılan yapılsın**" notu düşüldü.
  - Gerekçe: qBittorrent WebUI / port-fallback / şifre migration / seeding durumu `embedded_mode`'da
    yok; bu özellikler kaybolmadan önce Windows derlemesi teyit edilmeli. Wrapper'da unix'e özgü API
    yok → Windows derlemesi bekleniyor ama sandbox'ta kanıtlanmadı.
  - Uygulama: `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` (Faz 10b opt-in kullanım + ERTELENMİŞ KARAR)
    ve `docs/features/FEATURES.md` (changelog) güncellendi. Memory DECISION kaydı: librqbit default
    deferred-until-Windows-verified.

- 2026-08-12 — **KARAR UYGULANDI:** Python bridge varsayılan; librqbit opt-in + belgelendi;
  "Windows'ta derlendiğinde varsayılan yapılsın" notu roadmap + FEATURES.md'ye işlendi. TASK done.
