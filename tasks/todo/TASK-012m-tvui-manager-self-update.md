# TASK-012m — Manager-rs self-update (native, Python'sız)

- **id:** TASK-012m
- **title:** Manager binary self-update (version check + download + SHA256 verify + platform-safe apply + restart)
- **status:** in-progress
- **priority:** P1
- **created:** 2026-08-22
- **updated:** 2026-08-22
- **environment:** both
- **tags:** tvui, update, self-update, native, faz12b, parity
- **source:** `ports/RGSX/tvui.py` (`network.check_for_updates`, `apply_pending_update`); TASK-012h karar notu.
- **depends_on:** TASK-012h
- **supersedes:** none

## Kaynak

- `ports/RGSX/tvui.py:36` — `from network import ... check_for_updates, apply_pending_update ...`
- Python'da iki ayrı OTA: uygulama/manager binary (`pending_update_version`,
  `startup_update_confirmed`, `update_checked`) ve gamelist verisi (`gamelist_update_prompted`).
  Bu görev **manager binary** tarafı.
- TASK-012h SSE altyapısı (`catalog_update` eşleniği) → `manager_update` SSE'iyle tekrar kullanılır.
- `manager-http/src/self_update.rs` (Faz 1-4 implemente edildi), `manager-http/src/api.rs`,
  `manager-tvui/src/net.rs`, `manager-tvui/src/sdl2_shell.rs`, `webui/src/App.vue`.

## Açıklama

Manager binary'si kendini günceller; TVUI'de **onay + progress** ile görünür. Python parity
hedefi, ama son kullanıcı senaryosu (TVUI öncelikli, WebUI zorunlu değil, indirme arka planda)
doğrultusunda bilinçli sapmalar var.

### Faz 1-4 (TAMAM — commit ff4c45b, 2a2e411)
- Manifest kaynağı: yalnızca env `RGSX_UPDATE_MANIFEST_URL` (hardcoded yok). Boşsa no-op.
- `check_update` → `CARGO_PKG_VERSION` ile karşılaştır → yeniyse SSE `manager_update`
  `{available,version,url,sha256}` + `StateData.manager_update`.
- `download_and_verify` → temp dosyaya indir + SHA256 doğrula, **üzerine yazma YOK**.
- TVUI: banner + `Enter` → `POST /api/manager-update/download` → "indirildi: <path>".
- Canlı duman testi (entegrasyon): `manager-http/tests/self_update_smoke.rs` yeşil.

### Faz 5 (TASARIM — bu görev)
Geri alınamaz adım: indirilen temp binary'yi **çalışan exe'nin yerine koy + relaunch**.
İki ayrı karar noktası çözüldü:

| # | Karar | Sonuç |
|---|-------|-------|
| 1 | İki adımlı UI onayı (download → apply) | **Hayır, ayrı "emin misin?" dialog'u yok.** Tek tık = eylem. `Enter` indirmeyi başlatır (arka plan); hazır olunca `Enter` = apply (doğrudan, geri alınamaz). Tıklama yetkidir. |
| 2 | Servis senaryosu | Normal process varsayılır. **Serviste (systemd/Windows Service) `apply` reddedilir + log.** |
| 3 | Rollback | **`.old` yedek + `manager-bin --recover`** (`.old`'u geri koyar). Watchdog YOK. |
| 4 | WebUI self-update banner | **Bloker DEĞİL.** TVUI öncelikli; WebUI banner isteğe bağlı, Faz 5'i engellemez. |

**Kullanıcı ek refineler:**
- **Kuyruk + iptal:** Self-update indirmesi ayrı özel akış değil, **indirme kuyruğu öğesi**
  olmalı (WebUI/Python TVUI parity). `Enter` → binary indirme **kuyruğa girer** (progress SSE
  ile görünür), yanlış tık → **kuyruktan iptal** (`cancel`/`queue-remove`).
- **Tek-tık modeli:** misclick → kuyruktan iptal ederim (oyun indirmesi gibi). Apply tek tık =
  geri alınamaz replace; kullanıcı tercihi olarak kabul edildi.

### TVUI-prioritli akış (final)
```
[Boot/arka plan] check_update() → SSE manager_update {available,version,current}

[TVUI] Banner: "Güncelleme vX.Y mevcut"
  ├─ Enter (1) → POST /api/manager-update/download
  │              → KUYRUĞA girer (HTTP, cancellable, progress SSE)
  │              → kullanıcı grid'e döner, oyuna devam eder
  └─ İndirme bitince banner: "vX.Y hazır — uygula"
        └─ Enter (2) → POST /api/manager-update/apply
                        → replace + relaunch (geri alınamaz)
                        → kısa "Yeniden başlatılıyor…" → yeni process
```
State machine (TVUI): `idle → available → downloading(in queue) → ready → (Enter) applying → (yeni process) idle`.
Escape/yok say → banner kalır, apply olmaz (indirme devam eder/iptal edilir).

### Python parity notu
Python tek onayla indir+uygula+restart yapar; native bilinçli olarak **indirmeyi arka plana**
alıp apply'ı ayrı tık yaptık (indirme uzun→arka plan, apply geri alınamaz→bilinçli tık). Kullanıcı
"ayrı emin misin dialog'u istemiyorum" dedi → o dialog kaldırıldı; tıklama yetki sayılır.

## API sözleşmesi (Faz 5)
- `POST /api/manager-update/download` → indirmeyi **kuyruk görevi** olarak başlatır
  (`task_type: "manager_update"`, HTTP). Mevcut dönen `{success,path}` korunur; ayrıca
  `progress`/`queue` SSE ile ilerleme gelir, `cancel`/`queue-remove` ile iptal edilebilir.
- `POST /api/manager-update/apply` → `{success}`; replace + relaunch. Serviste reddedilir.
- SSE `manager_update`:
  - `{available, version, current}`
  - `{stage:"download", received, total}` (kuyruk progress)
  - `{stage:"ready"}` (apply bekliyor)
  - `{stage:"applying"}`

## Kapsam / Dosyalar (implementasyon planı)

**Güvenli parçalar (geri alınamaz değil — önce bunlar):**
1. Self-update indirmesi = **kuyruk görevi** (HTTP, `task_type:"manager_update"`),
   cancellable; mevcut `progress`/`queue` SSE + `cancel`/`queue-remove` ile çalışır.
   (`manager-http/src` queue worker + `self_update.rs`)
2. İndirilen temp yolu `StateData.manager_update["path"]` içinde saklansın (apply için).
3. `POST /api/manager-update/apply` handler → `self_update::apply_update(path)`;
   servis tespitinde reddet + log.
4. TVUI state machine + tek-`Enter` apply, "Yeniden başlatılıyor…" ekranı
   (`manager-tvui/src/net.rs`, `sdl2_shell.rs`).
5. SHA + versiyon kapıları (zaten mevcut, apply'da da zorunlu).

**Geri alınamaz parça (yalnız açık "evet" ile):**
6. `self_update::apply_update` (Win/Linux):
   - *Windows:* yeni exe temp'e → ayrı updater süreci (`cmd /c`: 1s bekle → temp→hedef `move`
     → hedefi `start`) → mevcut process çıkar (kilit çıkışta serbest).
   - *Linux:* `rename(temp, current)` (inode güvenli) → `execve` self-replace (aynı PID).
   - `.old` yedek; `manager-bin --recover` CLI flag'ı (`.old`'u geri koyar).

## Doğrulama

- Birim: versiyon parse/SHA (var), kuyruk görevi cancel, apply_update yol mantığı.
- Duman (Faz 1-4): mock manifest + headless manager-bin → SSE/endpoint.
- Duman (Faz 5): "replace sonrası yeni versiyon serviste mi?" + bozuk binary → `--recover`.
- WebUI update banner: bloker değil; sonra eklenebilir.

---

## İlerleme

- 2026-08-22 — TASK-012h kararı sonucu açıldı.
- 2026-08-22 — Faz 1-4 tamam (ff4c45b, 2a2e411): manifest/env, check_update, download+SHA,
  TVUI banner+download, canlı duman entegrasyon testi.
- 2026-08-22 — Faz 5 tasarımı netleşti (5 kullanıcı kararı: tek-tık uygula, serviste reddet,
  .old+--recover, WebUI bloker değil, kuyruk+iptal parity). Plan `tasks/` altına yazıldı.
- 2026-08-22 — Faz 5 implementasyon (güvenli parçalar 1-5 + apply kapısı): `self_update.rs`'te
  streaming/iptal edilebilir kuyruk indirmesi (`queue`+`progress` SSE, `/api/queue/remove` ile
  iptal) + `manager_update_apply` handler (servis reddi + `RGSX_SELF_APPLY=1` gate ile Win/Linux
  replace+relaunch + `.old` yedeği). TVUI: stage makinesi, Enter=indir/uygula, C=iptal,
  "Yeniden başlatılıyor…" ekranı. manager-http/tvui/bin derlendi; testler yeşil (28 lib + 19 tvui
  + smoke). **Commit edilmedi** (kullanıcı açık onayı bekleniyor; apply kapısı default kapalı).
