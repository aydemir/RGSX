# TASK-002h — Faz 10b: librqbit Windows cross-compile doğrulaması

- **id:** TASK-002h
- **title:** librqbit embedded engine'in Windows'ta derlendiğini doğrula (ertelenmiş varsayılan karar tetikleyici)
- **status:** done
- **priority:** P1
- **created:** 2026-08-12
- **environment:** both
- **tags:** rust, librqbit, cross-compile, windows, faz-10b
- **parent:** TASK-002

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10b (ERTELENMİŞ KARAR).
- TASK-002g kararı (2026-08-12): "librqbit, **Windows'ta derlendiği doğrulanınca** varsayılan
  motor yapılacak." Bu görev o doğrulamayı yapar.

## Açıklama

`manager-torrent` (librqbit 8.1.1) + `manager-bin` zincirinin Windows hedefinde derlenip
derlenmediğini kanıtla. aarch64 Linux sandbox'ta `cargo check --target x86_64-pc-windows-gnu`
ile doğrulanır. Wrapper'ımızda unix'e özgü API yok → derlenmesi bekleniyor; bu görev onu
ispatlar.

Başarılırsa: TASK-002g'deki ertelenmiş karar **tetiklenir** → librqbit varsayılan motor yapılır
(`resolve_engine` dalı değişir) ve memory/roadmap güncellenir. Başarısızsa (toolchain/linker
engeli): yalnız sandbox sınırı olduğu belgelenir, dev makinesine bırakılır; karar tetiklenmez.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/` (librqbit wrapper)
- `manager-rs/manager-bin/src/main.rs` — `resolve_engine`
- Hedef: `x86_64-pc-windows-gnu`

## Doğrulama

- `cargo check --target x86_64-pc-windows-gnu -p manager-torrent` (ve mümkünse tüm workspace)
  hatasız tamamlanır.
- Derleme sonrası varsayılan karar uygulanır (python → librqbit), roadmap + memory güncellenir.

## İlerleme

- 2026-08-12 — Tanımlandı (kullanıcı onayı: "onaylıyorum").
- 2026-08-12 — **Windows cross-compile ✅**: `mingw-w64` + `nasm` kuruldu; `cargo check
  --target x86_64-pc-windows-gnu -p manager-torrent` ve tüm workspace (`--workspace`,
  manager-bin + tray/autostart/firewall cfg(windows) dahil) hatasız derlendi (~4.5 dk).
  Wrapper'da unix'e özgü API yok → librqbit Windows'ta derlendi (doğrulandı).
- 2026-08-12 — **ERTELENMİŞ KARAR TETİKLENDİ → varsayılan çevrildi**: `resolve_engine`
  (manager-bin/src/main.rs) varsayılan dalı Python bridge → **librqbit** olarak değişti;
  Python bridge artık `RGSX_TORRENT_ENGINE=python` ile opt-in. Native `cargo test --workspace`
  114/114 geçti. Roadmap Faz 10b + FEATURES.md güncellendi; memory DECISION (SUPERSEDES
  mem_1786554601113_ckqgqns8i) yazıldı. TASK done.
