# TASK-013 — qBittorrent emekliliği: `/api/qbittorrent/*` + Python bridge engine sökümü

- **id:** TASK-013
- **title:** librqbit tek torrent yolu — qBittorrent uyumluluk katmanının (HTTP uçları + Python bridge istemcisi) sökülmesi
- **status:** done
- **priority:** P1
- **created:** 2026-08-25
- **environment:** both
- **tags:** qbittorrent-retirement, librqbit, bridge, cleanup
- **relation:** TASK-012-gap-02 önkoşul 2'nin icrası (diğer önkoşullar ayrı duruyor)

## Kaynak

Kullanıcı kararı (2026-08-25): "qbittorrent'i emekli edelim, planlarda otomatların kafası
karışıyor." gap-03 Faz C'de de qBittorrent bölümü aynı gerekçeyle düşmüştü. Torrent engine
default'u librqbit (TASK-002f/g); `/api/qbittorrent/*` uçları yalnız legacy
`RGSX_TORRENT_ENGINE=python` altında anlamlı.

## Kararlar (kullanıcı, 2026-08-25)

1. `manager-bridge`'deki Python `Bridge` istemcisi **tamamen sökülür** (üretim çağıranı kalmaz).
2. `saves\ports\rgsx\qbittorrent-portable` runtime klasörü **diskten silinir**.
3. `RGSX_TORRENT_ENGINE` env'i artık yok sayılır — librqbit tek yol (log güncellenir).

## KALACAKLAR (dokunulmaz)

- `TorrentBackend` trait + `BridgeError` + `ExtractHint`/`ProgressEvent` (engine-bağımsız;
  librqbit + manager-http `bridge_call` + contract fake'leri kullanıyor)
- `get_app_paths` trait metodu (tray main.rs:474 + api.rs:683/2322 tüketicili)
- pause/resume/cancel/cancel_all/is_paused/download_torrent(_progress) metodları
- `manager-torrent` (librqbit) crate'i
- `secrets.rs` qbittorrent_webui_password redaction (eski config/history koruması)
- `ports/RGSX/` Python port (donuk referans, `python-skeleton-final` tag)
- `resolve_script()` (main.rs) — tray ikonu anchor'ı olarak yaşar

## Fazlar

### Faz A — HTTP uçları
- [x] lib.rs: 4 route (`/api/qbittorrent/{change-password,start,password-status,regenerate-password}`)
- [x] api.rs: 4 handler (`change_password`, `qb_start`, `qb_password_status`, `qb_regenerate_password`)
- [x] contract.rs: qbittorrent uç testleri — gerçek baseline **114 → 105**
      (ilk oturum 4 test sildi; ikinci oturum atlanan 5 `test_qb_*` placeholder
      testini daha bulup sildi — 110 hedefi yanlıştı, doğru sayı 105)
- [x] main.rs log satırı "qbittorrent proxy" ibaresi

### Faz B — Python engine yolu
- [x] main.rs `resolve_engine`: `RGSX_TORRENT_ENGINE=python` dalı sökülür; librqbit tek yol
- [x] main.rs header doc (TASK-002c anlatımı) + shutdown yorumu güncellendi
- [x] paths.rs: `manager_script` alanı + `qbittorrent_backend.py` çözümü söküldü
      (`RGSX_MANAGER_SCRIPT` env artık set edilmez; `resolve_script` tray anchor olarak kalır)
- [x] manager-bridge: `Bridge` struct + `impl TorrentBackend for Bridge` + spawn/read_loop/
      drain_stderr/Pending/BridgeConfig + typed metodlar + python-echo testleri
- [x] trait'ten qbittorrent-kavramlı default'lar: `is_available`, `ensure_running`,
      `get_webui_url`, `get_password_status`, `change_webui_password`,
      `regenerate_qbittorrent_password`, `ping`, `status` (+ `BridgeStatus` struct)
- [x] (2. oturum eki) manager-torrent `call()` dispatch'inden öksüz kollar (`ping`/`status`/
      `is_available`/`ensure_running`/`get_webui_url`/`get_password_status`/
      `change_webui_password`) + tüketicisiz kalan `is_running()` söküldü;
      engine.rs'e "emekli metodlar -32601 döner" regresyon testi eklendi

### Faz C — docs + runtime
- [x] PROJECT_MAP contract sayıları 105 + qbittorrent satırları güncellendi
- [x] gap-02 önkoşul 2 → "TASK-013 ile yapıldı"
- [x] `saves\ports\rgsx\qbittorrent-portable` silindi (52 dosya, 53.2 MB)
- [x] manager-torrent/state.rs qbittorrent ibareleri tarihsel notuya çevrildi
      (manager-core state.rs BackendState yorumu dahil)

## Doğrulama

- `cargo test --workspace` yeşil (contract baseline 105) ✓
- `grep -rni qbittorrent manager-rs --glob "*.rs"` → yalnız secrets redaction + tarihsel
  doc notları (arşiv ibaresiyle) ✓
- Canlı: manager-bin açılır, "torrent engine: librqbit" logu, indirme akışı (DDL) etkilenmez

## İlerleme

- 2026-08-25 — Görev açıldı; gap-02 önkoşul 2 bu dosyaya devredildi. Faz A+B kısmi uygulama
  (sağlayıcı hatası yüzünden oturum yarıda kesildi).
- 2026-08-26 — Tamamlama: manager-torrent öksüz call() kolları + is_running sökümü,
  engine.rs regresyon testi, contract.rs'ta atlanan 5 test_qb_* placeholder testinin silinmesi
  (gerçek baseline 105), state.rs tarihsel notu, Faz C (PROJECT_MAP + gap-02 + portable silme).
  Not: HEAD'de pre-existing uyarılar (paths.rs never-read alanları, http_integration
  StreamBody/out) TASK-005 kapsamında bırakıldı.
- 2026-08-26 — Canlı smoke ✓: taze debug binary boot, "torrent engine: librqbit (embedded,
  tek yol)" logu, /api/health 200, tray açıldı. Gözlem: sökülen yollar dahil bilinmeyen
  TÜM istekler genel SPA fallback'inden text/html 200 alıyor (pre-existing router tasarımı;
  ileride /api/* için 404 JSON ayrımı düşünülebilir — yeni görev adayı).
