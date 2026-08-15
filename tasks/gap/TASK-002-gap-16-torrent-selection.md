# TASK-002-gap-16 — Torrent Dosya Seçimi + Öncelik (librqbit'te eksik)

- **id:** TASK-002-gap-16
- **title:** Multi-file torrent file selection + priority (librqbit AddTorrentOptions)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** torrent, librqbit, selection
- **parent:** TASK-002

## Karar (2026-08-15)

`/api/download` torrent request contract değişmez. Ancak çok dosyalı torrent'te Python
hedef dosyayı seçip öncelik verirken Rust **tüm dosyaları indirir**. Hedef dosya seçimi
gerekli (yanlış/gereksiz veri indirmeyi önler).

> BELİRSİZ: hangi dosyanın "hedef" olduğu nasıl belirlenir? Python `qbittorrent_backend.py:954-1016`
> `_apply_file_selection` mantığı (en büyük dosya? `is_zip_non_supported`? game_name eşlemesi?)
> librqbit `AddTorrentOptions` ile nasıl eşlenir — uygulamaya başlamadan kullanıcı onayı gerekir.

## Python Referans Davranışı

- `ports/RGSX/qbittorrent_backend.py:954-1016` (`_apply_file_selection`, `filePrio`) — çok dosyalı
  torrent'te hedef dosya seçimi + priority (0/1).
- `ports/RGSX/qbittorrent_backend.py:1556` savepath `.rgsx_torrent/<key>`; `:1675-1681` kaynağı
  koruyup yalnız seed için link.

## Rust Mevcut Durum (❌ / ⚠️)

- `manager-torrent/src/lib.rs:91` `AddTorrentOptions::default()` — **tüm dosyaları indirir**,
  dosya seçimi YOK.
- `manager-torrent/src/lib.rs:73` `output_folder`; `:321-368` en büyük dosyayı seçip `link_or_copy`
  — temp_dir izolasyonu ve seed için kaynak koruma YOK (bkz. TASK-002-gap-8).
- Dosya önceliği (priority 0/1) YOK.

## Kapsam / Dosyalar (değişecek, implementasyona başlamadan doğrulanacak)

- `manager-rs/manager-torrent/src/lib.rs:85-102` `add_torrent` — `AddTorrentOptions` file filters/priority
- `manager-rs/manager-torrent/src/lib.rs:73,321-368` — disk yazma izolasyonu (gap-8 ile paylaşımlı)

## Bağımlılık

- Yok (bağımsız). `TASK-002-gap-8` (stray temp / izolasyon) ve `TASK-002-gap-7` (seed) ile
  `manager-torrent/src/lib.rs` dosyalarını paylaşır — aynı dosyada çakışmamak için sıralı ele alınmalı.

## Doğrulama

- Çok dosyalı torrent: yalnız hedef dosya indirilir, gereksiz dosya atlanır (Python parity).
- Dosya önceliği uygulanır.
- İndirme sonrası temp izolasyonu + kaynak koruma (gap-8) ile birlikte doğrulanır.
