# TASK-012m — Manager-rs self-update (native, Python'sız)

- **id:** TASK-012m
- **title:** Manager binary self-update (version check + download + SHA256 verify + platform-safe apply + restart)
- **status:** todo
- **priority:** P1
- **created:** 2026-08-22
- **environment:** both
- **tags:** tvui, update, self-update, native, faz12b, parity
- **source:** ports/RGSX/tvui.py (`network.check_for_updates`, `apply_pending_update`); TASK-012h karar notu (kullanıcı: "manager-rs ye gelen güncellemeler nasıl olmalı")
- **depends_on:** TASK-012h
- **supersedes:** none

## Kaynak

- `ports/RGSX/tvui.py:36` — `from network import ... check_for_updates, apply_pending_update ...`
- `config.pending_update_version`, `config.update_checked`, `config.gamelist_update_prompted`,
  `config.startup_update_confirmed` — Python'da güncelleme onay akışı flag'leri.
- TASK-012h SSE altyapısı (`catalog_update` eşleniği) — aynı desen `manager_update` SSE'iyle tekrar kullanılır.

## Açıklama

Python'daki manager self-update Rust'e portlanır; TVUI/WebUI'de **onay + progress bar** ile
görünür (Python parity). İki bağımsız akış vardı; katalog OTA TASK-012h'ye katıldı, bu görev
**manager binary'sinin kendini güncellemesi**.

**Akış:**
1. **Versiyon kaynağı:** JSON (`RGSX_UPDATE_URL` veya GitHub release latest) →
   `{version, url_windows, url_linux, sha256}`. Mevcut versiyon `manager-bin` build metadata'ından.
2. **Kontrol:** başlangıçta (arka plan) + periyodik → SSE `manager_update`
   `{available, version, current}`.
3. **Uygulama (TVUI onayı):** binary indir + SHA256 doğrula + **platform-safe** değiştirme:
   - Linux: çalışan binary'yi değiştir → restart.
   - Windows: çalışan `.exe` overwrite edilemez → küçük updater/relaunch deseni (yeni exe'yi
     temp'e al, kapatınca swap + restart). `manager-windows` (NSIS) bağlamıyla uyumlu.
4. **TVUI:** `update_prompt` state → "Güncelleme mevcut vX.Y" → bas → `manager_update` SSE barı
   (`{stage:"download", received,total}`) → "Yeniden başlatılıyor".

**Behavior contract (parity):**
- Güncelleme mevcutsa TVUI'de bir kez sorulur (Python `gamelist_update_prompted` flag parity).
- İndirme sırasında progress bar akar (SSE, 250ms throttled değil — tek akış yeterli).
- Doğrulama başarısızsa eski binary korunur (rollback yok, ama zarar görmez).

## Kapsam / Dosyalar

- `manager-update` yeni crate (veya `manager-bin` içinde mod): version check, download, verify, apply.
- `manager-http/src` güncelleme endpoint'i (`/api/check-update`, `/api/apply-update`) + SSE `manager_update`.
- `manager-tvui/src/` `update_prompt` state + bar render.
- `manager-windows` Windows relaunch/updater yardımcısı.

## Doğrulama

- Versiyon JSON parse + SHA256 doğrulama birim testi.
- Linux: sahte yeni binary ile swap + restart simülasyonu.
- Windows: exe overwrite engeli relaunch deseni ile aşılır (integration).

---

## İlerleme

- 2026-08-22 — TASK-012h kararı sonucu açıldı (kullanıcı onayı: "doğru anlamışsın önerini kabul ediyorum").
