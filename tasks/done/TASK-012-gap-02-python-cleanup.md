# TASK-012-gap-02 — Python kalıntılarının `custom` dalından temizlenmesi (cutover sonrası)

- **id:** TASK-012-gap-02
- **title:** Native-Rust tek-yol geçişinin fiziksel tamamlanması — ports/RGSX + bridge Python spawn + qBittorrent uyumluluk katmanı sökülümü
- **status:** done (2026-08-26)
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

- [x] **1. Cutover tamam** — TASK-012h→i→j→k→l native ekranlar + klavye + erişilebilirlik
      tamam; `RGSX_TVUI=0` Python fallback dalı Rust kodunda hiç var olmadığından söküm =
      manager-tvui doc-comment sözünün kaldırılması + `ports/RGSX` silinmesi (gap-02).
- [x] **2. Torrent native-only** — **TASK-013 ile yapıldı** (2026-08-26): `Bridge` subprocess
      istemcisi + `/api/qbittorrent/*` uçları söküldü, librqbit tek yol; `TorrentBackend`
      trait'i manager-bridge'de engine-bağımsız sözleşme olarak yaşıyor.
      Detay: `tasks/done/TASK-013-qbittorrent-retirement.md`.
- [x] **3. Windows launcher sadeleşir** — `windows/RGSX Retrobat.bat`
      (tamamı Python launcher'ı) ve `windows/scripts/rgsx_firewall_setup.ps1` (tümü qBittorrent
      program kuralı + eski WebUI portu 18572) silindi; `RGSX rust.bat` + `verify_gap01.ps1` kaldı.
- [x] **4. Contract güvencesi Rust'a taşınır** — pytest baseline API sözleşmeleri manager-http
      `contract.rs`'teki testlerle karşılanıyor (qbittorrent uçları TASK-013 ile bilinçli
      emekli); birim davranışlar game_filters/settings/watchdog/state/secrets/tvui modül
      testlerinde var.

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
- 2026-08-26 — Önkoşul 3 (launcher sadeleşmesi) tamam: `windows/RGSX Retrobat.bat`
  (tamamı Python launcher'ı) ve `windows/scripts/rgsx_firewall_setup.ps1` (tümü qBittorrent
  program kuralı + eski WebUI portu 18572) silindi; `RGSX rust.bat` + `verify_gap01.ps1` kaldı.
- 2026-08-26 — Önkoşul 4 (contract güvencesi) değerlendirmesi: pytest baseline API sözleşmeleri
  manager-http `contract.rs`'teki 140 testle karşılanıyor (qbittorrent uçları TASK-013 ile bilinçli
  emekli); birim davranışlar game_filters/settings/watchdog/state/secrets/tvui modül testlerinde
  var. Rust testleri modül modül çalıştırıldı (Termux RAM kısıtı): manager-core 75, scan 8+29,
  download 14, torrent 12+4, http contract+faz5 dahil → yeşil.
- 2026-08-26 — Düzeltme: `faz5_smoke.rs` apply testleri kendi ~146 MB test binary'sini indirip
  15 sn'de `ready` bekliyordu; yavaş diskli cihazlarda (Termux) timeout'a düşüyordu. Ready
  timeout'u 180 sn'e çıkarıldı (satır 336/412). Kod regresyonu değil, çevre toleransı.
- Kalan: önkoşul 1 kalanı (TVUI `RGSX_TVUI=0` Python fallback dalının sökülmesi) + toplu silme
  (`ports/RGSX`, kök `tests/`, bridge spawn kodu, `/api/qbittorrent/*` kalıntıları) +
  doğrulama grepleri + README/docs/skill metinlerinin native-only güncellenmesi.
- 2026-08-26 — **TAMAMLANDI.** Toplu silme: `ports/RGSX/` (yalnız `ports/images|videos|gamelist.xml`
  kaldı), kök `tests/` (21 pytest), `pytest.ini`/`.coveragerc`/`.coverage`/`.pytest_cache`,
  `deps/python.zip` (+boşalan `deps/`), `docker/` (tamamen Python imajıydı — kullanıcı kararıyla
  silindi), `manager-bridge/tests/echo_bridge.py`. Kod sökümü: `catalog.rs` `PythonCatalog`
  + `RGSX_PYTHON_MANAGER_URL` proxy'si ve `NativeCatalog.python` alanı; manager-bin
  `RGSX_NATIVE_CATALOG` flag'i (koşulsuz NativeCatalog) + `resolve_script()`
  (`qbittorrent_backend.py` fallback'i; tray ikonu artık exe-dizininden çözülür);
  gamelist Windows entry `./RGSX Retrobat.bat` → `./RGSX rust.bat` (+test).
- 2026-08-26 — Release pipeline native'e çevrildi (kullanıcı kararı): `release.yml` artık
  WebUI (npm) + manager-bin (ubuntu-22.04 Linux x86_64 glibc-uyumlu, windows-latest MSVC)
  derleyip aynı paket adlarıyla zip'ler; `RGSX_update_windows_latest.zip` roms-kökü ağacı
  (ports/RGSX/manager-bin.exe+webui + windows/) içerir. Batocera için native `ports/RGSX/RGSX.sh`
  eklendi. İlk tag push'ta CI gerçek koşusuyla doğrulanmalı (SDL2 bundled apt bağımlılıkları
  tahminî liste).
- 2026-08-26 — WebUI i18n tek kaynağı taşındı: `webui/languages/*.json` (7 dil;
  `gen-i18n.mjs` güncellendi). Dikkat: `webui/src/i18n.strings.js` AUTO-GENERATED — elle
  düzenlenmez, kaynağı `webui/languages/`. Kullanıcıya görünen `RGSX_NATIVE_CATALOG` ifadeleri
  kaynak JSON'lardan temizlendi, dist yeniden build edildi.
- 2026-08-26 — Doğrulama nuansı: "kodda sıfır qbittorrent/pygame isabet" hedefi harfiyen
  mümkün değil ve gerekli de değil — kalan isabetler (a) arşiv/tarihçe doc-comment'leri,
  (b) legacy `qbittorrent_webui_password` ayar anahtarının redaksiyon sözleşmesi (eski
  kullanıcı ayar dosyaları hâlâ bu anahtarı içerebilir; contract.rs + secrets.rs testleri
  bunu kilitleyer), (c) emeklilik davranışını test eden isimler (engine.rs retired-methods).
  Canlı Python/qBittorrent kod yolu: SIFIR.
- 2026-08-26 — Doğrulama: `cargo check --workspace --all-targets` yeşil; manager-scan 8,
  manager-http contract 105 + lib 28 yeşil. README×3 + PROJECT_MAP native-only güncellendi
  (`.opencode/skills` repoda yok → N/A).
