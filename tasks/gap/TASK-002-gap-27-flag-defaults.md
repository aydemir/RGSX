# TASK-002-gap-27 — manager-bin flag defaults (saf-Rust varsayılanları)

- **id:** TASK-002-gap-27
- **title:** manager-bin flag defaults (saf-Rust varsayılanları + bağımsızlık korunsun)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-15
- **environment:** both
- **tags:** manager-bin, flags, launcher, refactor
- **parent:** TASK-002

## Karar (2026-08-15) — onaylanan plan

- manager-bin, flag varsayılanlarını "saf-Rust" değerine çeker; her flag YİNE env ile override
  edilebilir. **Flag'ler BAĞIMSIZ tutulur — tek `RGSX_STANDALONE=1`'e indirgenMEZ.**
- Launcher (`RGSX rust.bat`) bu flag'leri set etmeyi bırakır (gap-26 ile birlikte); yalnız runtime
  flag'ları (`RGSX_TVUI`, `RGSX_DISPLAY`, `RGSX_WINDOWED`, `--windowed`/`--no-tvui`) kalır.

## Bağımsızlık kanıtı (tek bayrağa indirgeme REDDEDİLDİ)

Her flag farklı koddan, ortogonal okunuyor:
- `RGSX_RUST_WEBUI` → port seçimi (`manager-bin/src/main.rs:194`)
- `RGSX_NATIVE_CATALOG` → katalog kaynağı (`main.rs:224`)
- `RGSX_TORRENT_ENGINE` → python bridge vs librqbit (`main.rs:50`)
- `RGSX_NATIVE_DOWNLOAD` → native DDL yolu (`manager-http/src/api.rs:415`)
- `RGSX_NO_AUTOSTART` → Windows autostart kaydı (`main.rs:105`)

Gerçek hibrit senaryo: native catalog (=1) + python torrent bridge (`TORRENT_ENGINE=python`) — kod
bunu destekliyor. Tek bayrak bu kombinasyonu öldürür. ➡️ Bağımsızlık korunur.

## Yeni varsayılanlar (env yoksa)

| Flag | Eski varsayılan | Yeni varsayılan | Gerekçe |
|---|---|---|---|
| `RGSX_RUST_WEBUI` | `false` (port 5010) | `true` (port 5000) | saf-Rust launcher |
| `RGSX_NATIVE_CATALOG` | `false` (Python proxy) | `true` | saf-Rust katalog |
| `RGSX_TORRENT_ENGINE` | librqbit (zaten) | librqbit | değişmez |
| `RGSX_NATIVE_DOWNLOAD` | `false` | `true` | native DDL açık |
| `RGSX_NO_AUTOSTART` | `false` | `false` (override edilebilir) | saf-Rust'ta autostart istenebilir; .bat yine `=1` set edebilir |

Her biri `std::env::var(...).map(|v| v=="1").unwrap_or(<yeni_default>)` ile okunur.

## KAPSAM DIŞI

- Çift-"roms" off-by-one (`RGSX rust.bat:89`) → gap-26 KAPSAM DIŞI; koşullu `gap-28` (bkz. gap-26).
- `RGSX_DISPLAY` / `RGSX_WINDOWED` / `RGSX_TVUI` / `--windowed` / `--no-tvui` → runtime niyet,
  bu TASK'a dokunulmaz; launcher'da kalır (gap-26 ile koordineli).

## Anchor-fallback warn log (gap-26 ile paylaşımlı görünürlük şartı)

`manager-bin/src/paths.rs` içindeki anchor-fallback `tracing::warn!` (gap-26 Kapsam) **bu TASK ile
birlikte geçerlidir**. `RGSX_NATIVE_CATALOG` ve `RGSX_NATIVE_DOWNLOAD` default'ları artık `true`
olduğundan, anchor tespiti custom kurulumda başarısız olup yanlış path türetirse sistem eskisi gibi
Python proxy'ye sessizce düşmez; yanlış dizinde native modda çalışmayı dener. Bu nedenle fallback
anında basılan warn log kritiktir — kök sebep (yanlış türetilen `root`) log'suz kalırsa boş katalog /
eksik dosya gibi dolaylı belirtilerle geç ortaya çıkar.

## Rust Mevcut Durum (dosya:satır)

- `manager-bin/src/main.rs:194-195` `RGSX_RUST_WEBUI` → `default = if rust_webui {5000} else {5010}`.
- `manager-bin/src/main.rs:224-226` `RGSX_NATIVE_CATALOG` → `unwrap_or(false)`.
- `manager-bin/src/main.rs:50` `RGSX_TORRENT_ENGINE` → `"python"` dışı librqbit (varsayılan OK).
- `manager-http/src/api.rs:415` `RGSX_NATIVE_DOWNLOAD` → `unwrap_or(false)`.
- `manager-bin/src/main.rs:105` `RGSX_NO_AUTOSTART` → `unwrap_or(false)`.

## Kapsam / Dosyalar (değişecek)

- `manager-bin/src/main.rs:194-200` — `RGSX_RUST_WEBUI` default `true`; `RGSX_MANAGER_BIN_PORT`
  default `5000` (port seçimi artık sabit 5000 olabilir, flag yine override eder).
- `manager-bin/src/main.rs:224-226` — `RGSX_NATIVE_CATALOG` default `true`.
- `manager-http/src/api.rs:415` — `RGSX_NATIVE_DOWNLOAD` default `true`.
- `manager-bin/src/main.rs:105` — `RGSX_NO_AUTOSTART` default `false` (değişmez; yorum güncellenir).
- `windows/RGSX rust.bat:204-209` — `RGSX_RUST_WEBUI`, `RGSX_NATIVE_CATALOG`, `RGSX_NATIVE_DOWNLOAD`,
  `RGSX_TORRENT_ENGINE`, `RGSX_NO_AUTOSTART`, `RGSX_MANAGER_BIN_PORT` set satırları düşürülür
  (varsayılanlar manager-bin içinde; override gerekirse elle verilir).

## Bağımlılık

- `TASK-002-gap-26` ile birlikte uygulanmalı (aynı senkron `main` prolog'u).

## Doğrulama

- Launcher'sız `manager-bin.exe`: port 5000, native catalog aktif, native download açık.
- `RGSX_NATIVE_CATALOG=0` set edilince Python proxy'ye düşer (override çalışır).
- `RGSX_TORRENT_ENGINE=python` + `RGSX_NATIVE_CATALOG=1` hibrit modu bozulmadan çalışır.
- `.bat` yalnız runtime flag'ları set eder; path/catalog/torrent env'leri artık yok.
- `RGSX_NATIVE_CATALOG` default `true` + anchor-fallback `tracing::warn!` (gap-26) birlikte: yanlış
  dizin tespiti log'suz kalmaz; boş katalog/eksik dosya gibi geç belirtiler engellenir.
