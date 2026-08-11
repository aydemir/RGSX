---
id: TASK-002a
title: manager-core state machine tasarımı (Faz 10a ilk adım)
status: done
priority: P2
created: 2026-08-11
tags: [rust, manager-core, state-machine]
parent: TASK-002
---

# TASK-002a — manager-core state machine tasarımı

## Kaynak

- **Parent:** TASK-002 (Faz 10 — Rust kısmi refaktör, "Sıralama: önce state machine (`enum`)")
- **Mevcut iskelet:** scaffold commit `dc3aa21` (boş `state.rs` / `contract.rs`)

## Açıklama

`rgsx_manager.py` ve `watchdog.py`'deki daemon state'lerini Rust `enum` + `match`
olarak `manager-core/src/state.rs`'e modellemek. Codegraph ile çıkarılan
**gerçek state kümeleri** aşağıdadır (tahmin yok):

### 1. Manager durumları — `ports/RGSX/watchdog.py:14-21`

| Sabit | Değer | Geçişi tetikleyen |
|---|---|---|
| `STATE_INIT` | `INIT` | başlangıç; watchdog başlatılınca `RESTARTING`→... |
| `STATE_RUNNING` | `RUNNING` | healthy |
| `STATE_DEGRADED` | `DEGRADED` | ardışık fail ≥ `degrade_threshold` (3) |
| `STATE_UNRESPONSIVE` | `UNRESPONSIVE` | ardışık fail ≥ `unresponsive_threshold` (6) |
| `STATE_RESTARTING` | `RESTARTING` | UNRESPONSIVE → spawn + kapanış |
| `STATE_CRASHED` | `CRASHED` | restart limiti aşıldı (RestartLimiter) |

Referans kod (watchdog.py satır 8-10 gerçek makine): `INIT → RUNNING ⇄ DEGRADED → UNRESPONSIVE → RESTARTING → CRASHED`

### 2. qBittorrent backend durumları — `watchdog.py:22-26`

`STOPPED`, `STARTING`, `PORT_RESOLVING`, `WEBUI_AUTH_WAIT`

### 3. İndirme (download) durumları — `network/download_state.py:30-42`

`QUEUED`, `DOWNLOADING`, `PAUSED`, `VERIFYING`, `EXTRACTING`, `RETRY_SCHEDULED`,
`FAILED_TRANSIENT`, `FAILED_PERMANENT`, `COMPLETED`, `CANCELED`

geçiş kararlarına eşlik eden saf mantık: `HysteresisMonitor` (degrade=3,
unresponsive=6), `RestartLimiter` (max=3, window=3600s) — `watchdog.py:29-97`.
Bunlar da Rust'a taşınacak (saf fonksiyon / struct).

Bu adım **SADECE state machine**: enum + match + transition fonksiyonları + saf
watchdog mantığı (hysteresis/restart sınırı). HTTP/tray/firewall/SSE entegrasyonu
kapsam dışı — TASK-002b/c/d alt-görevlerine bırakılır.

## Kapsam / Dosyalar

- `manager-rs/manager-core/src/state.rs` — üç enum (ManagerState, BackendState,
  DownloadState) + transition/logic
- `manager-rs/manager-core/src/contract.rs` — gerekirse state tipleri için
  serde uyumlu sözleşme tipleri (`str` eşleşmesi: `"INIT"`, `"RUNNING"`, …)
- `watchdog.py` saf mantık karşıtı: `HysteresisMonitor` / `RestartLimiter` Rust
  karşılığı (unit-test edilebilir saf struct'lar)

## Doğrulama

- Python'daki tüm state'ler enum'da karşılığını bulur — eşleştirme tablosu
  (yukarıdaki 3 küme) `#[cfg(test)]` içinde string↔enum eşleme testiyle kilitlenir
- Her state transition için en az 1 Rust unit test (`#[test]`)
- Hysteresis: ardışık fail 3→`DEGRADED`, 6→`UNRESPONSIVE`, healthy→sayaç sıfırla
  testleri; RestartLimiter: kayan pencere + limit dolunca `False`
- `cargo test -p manager-core` ve `cargo check -p manager-core --workspace` geçer
- Characterization karşılaştırması (Python testlerini Rust'a karşı çalıştırma)
  **bu adımda YOK** — TASK-002b'de

## İlerleme

- [x] **Adım 1 (commit `986d60d`):** üç enum + DownloadEvent, serde `SCREAMING_SNAKE_CASE`
      eşlemesi, `impl_status_str!` macro (Display/FromStr), legacy map fonksiyonları + testler
- [x] **Adım 2 (bu task):** `transition()` + `IllegalTransitionError` (download_state.py
      `_TRANSITIONS` 21 satır birebir, exhaustive match); `ALL_*` varyant listeleri;
      `watchdog.rs` yeni modül — `HysteresisMonitor` (degrade=3/unresponsive=6,
      healthy→reset) + `RestartLimiter` (max=3, window=3600s, kayan pencere, `max(1,…)`
      clamp); lib.rs `pub mod watchdog;`
- [x] **Testler:** 24 Rust unit test (`cargo test -p manager-core`) — string↔enum roundtrip,
      legacy eşleme, 21 geçerli transition tablosu + exhaustive "yalnızca 21 geçerli",
      hysteresis 3→DEGRADED / 6→UNRESPONSIVE / healthy reset, restart: limit dolunca False,
      kayan pencere prune, window sınırında atım
- [x] `cargo check --workspace` geçer (1m48s); hedef out-of-tree:
      `manager-rs/.cargo/config.toml` → target-dir `C:/Users/lv/RGSX/rust-target`
      (repo içi `target/` üretilmiyor — `.gitignore` da `manager-rs/target/` yedek)
- [x] Rust toolchain kuruldu: `rust-msvc` 1.97.1 (scoop)
- [x] Commit `5895315` + push `aydemir/custom` (`2f2e5cc..5895315`)
- [x] TASK-002a done'a taşındı
- [ ] Characterization (Python↔Rust) — TASK-002b