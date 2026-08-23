# TASK-012-gap-02 — Python kalıntılarının `custom` dalından temizlenmesi (cutover sonrası)

- **id:** TASK-012-gap-02
- **title:** Native-Rust tek-yol geçişinin fiziksel tamamlanması — ports/RGSX + bridge Python spawn + qBittorrent uyumluluk katmanı sökülümü
- **status:** todo (önkoşullar bekleniyor)
- **priority:** P2
- **created:** 2026-08-22
- **environment:** both
- **tags:** python-removal, cutover, librqbit, contract-tests, cleanup

## Kaynak

- Dal stratejisi kararı (2026-08-22): `main` = donuk Python referansı, `custom` = tek geliştirme
  hattı (`docs/PROJECT_MAP.md` §0). Kullanıcı "custom'daki Python'ları temizleyelim mi?" sorusuyla
  açıldı; cevap: **önkoşullar bitmeden hayır**.
- Güvenlik ağı: `python-skeleton-final` tag'i (custom HEAD, 2026-08-22) — Python'ın son tamam
  durumunu birebir tutar; temizlik sonrası geri dönüş/nedensellik sorgularının tek kaynağı.
  NOT: `origin/main` (ffcfcd4) BAYAT bir Python anlık görüntüsüdür (son 566 commit'te Python da
  değişti) — referans için main'e değil bu tag'e bakılmalı.

## Önkoşullar (hepsi bitmeden SİLME başlamaz)

- [ ] **1. Cutover tamam** — TASK-012h→i→j→k→l: TVUI native ekranlar + klavye + erişilebilirlik +
      cutover; `RGSX_TVUI=0` Python pygame fallback dalı kaldırılır (`manager-tvui/src/lib.rs`
      başlığındaki fallback sözü dahil).
- [ ] **2. Torrent native-only** — `manager-bridge::Bridge::spawn("qbittorrent_backend.py --bridge")`
      Python subprocess yolu sökülür (`manager-bin/src/main.rs` resolve_engine default'u zaten
      `librqbit`; Bridge spawn + `/api/qbittorrent/*` uçları `api.rs:1531-1605` kaldırılır).
      `TorrentBackend` trait'inin nerede yaşayacağı o gün kararlanır (manager-bridge daralır ya da
      manager-torrent'e taşınır).
- [ ] **3. Windows launcher sadeleşir** — `windows/RGSX Retrobat.bat` Python/qBittorrent satırları
      (:501-509 firewall bloğu dahil), `windows/scripts/rgsx_firewall_setup.ps1`'in QbittorrentPath +
      18572 bölümleri. `verify_gap01.ps1` gibi Rust araçları KALIR.
- [ ] **4. Contract güvencesi Rust'a taşınır** — kök `tests/`'teki 102 pytest baseline'ın karşıladığı
      API sözleşmeleri cargo testleriyle kapatılır (mevcut çekirdek: `faz5_smoke.rs`,
      `self_update_smoke.rs`, contract.rs; eksikler sayılıp yazılır). Bu olmadan silme güvenlik
      ağsızdır.

## Kapsam / Silinecekler

- `ports/RGSX/` (tüm Python uygulaması — display/controls/language/network/history/scraper…)
- Kök `tests/` pytest süiti (madde 4'te taşınmayanlar)
- `manager-rs/manager-bridge` Python spawn kod yolu (crate kaderi madde 2'de)
- `manager-http`: `/api/qbittorrent/{change-password,start,password-status,regenerate-password}`
- Windows launcher/PS1 qBittorrent bölümleri (yukarıda)
- Kontrol edilecekler: `docker/` içeriği Python'a bağımlıysa güncellenir; `deps/` içindeki Python
  runtime paketleri (yalnız Windows dağıtımı içinse) gider.

## Kalcaklar (dokunulmaz)

- `webui/` (Vue SPA), `manager-rs/` (native), `docs/`, `tasks/`, `.opencode/skills/`
  (rgsx-faz12-migration ve rgsx-contract-tests skill metinleri Rust-only hale göre güncellenir),
  `python-skeleton-final` tag'i.

## Doğrulama

- `grep -rni "qbittorrent\|pygame\|qbittorrent_backend"` → kodda sıfır isabet (yalnızca docs/tasks
  tarihçe notlarında kalabilir; her biri "(arşiv)" ibaresiyle).
- `cargo test` tüm workspace yeşil; Windows cross-check yeşil.
- Uçtan uca native kurulum: TVUI (gamepad) → katalog → DDL + torrent (librqbit) indirme canlı;
  Python süreç/hatı hiçbir akışta aranmaz.

## İlerleme

- 2026-08-22 — Görev oluşturuldu; `python-skeleton-final` tag'i atılıp pushlandı. Temizlik
  cutover + torrent native-only + launcher sadeleşmesi + contract taşıması bitince tek commit'te
  yapılacak (bu dosyanın checklist'i işaretlenerek).
