# TASK-002 — Faz 10: Rust kısmi refaktör (manager state machine + concurrency)

- **id:** TASK-002
- **title:** Faz 10 — Rust kısmi refaktör (EN SON)
- **status:** done
- **priority:** P2
- **created:** 2026-08-11
- **tags:** rust, rgsx-manager, qbittorrent-backend, network, concurrency

## Kaynak

- **Roadmap:** `docs/roadmap/ROADMAP_DOWNLOAD_MANAGER.md` — Faz 10 (Rust kısmi refaktör, "EN SON")

## Açıklama

**Amaç:** State machine + concurrency-ağır manager bileşenlerini Rust'a taşımak; Linux/Batocera
desteği kırılmadan.

**Motivasyon:** Rust'ta `enum` + `match` ile compiler-enforced state transition'ları; `librqbit`
(`rqbit` motoru) embedded qBittorrent'i ikame edebilir.

**Kısıt:** Bu faz ancak Faz 1-9 tamamlandıktan sonra — özellikle Faz 7 (characterization tests)
olmadan başlanamaz. Roadmap'in Önerilen Sıra bölümüne göre Faz 9 ile Faz 1-9 tamamlandı ve
**Faz 10 sıradaki aktif fazdır** (ancak Faz 7 karakterizasyon testleri olmadan başlanamaz).

**Platform bölünmesi (doğrulanmış kısıt):**

| Bileşen | Platform kapsamı | Rust'a geçiş |
|---|---|---|
| `rgsx_manager.py` (daemon, tray, autostart, port resolve, SSE, watchdog) | Windows-only | ✅ Faz 10a — risk düşük |
| `qbittorrent_backend.py` (embedded torrent, `librqbit` adayı) | Windows **+** Linux/Batocera | ⏸ Faz 10b — Linux/ARM test imkânı şart |

**Ara mimari:** Rust manager binary, mevcut `qbittorrent_backend.py`'yi subprocess olarak
çağırmaya devam eder (JSON-RPC veya local HTTP köprüsü). Windows tarafı kademeli Rust'a geçerken
Linux/Batocera Python'da kalır.

**Sözleşme:** `/api/*` ve `/api/events` (SSE) — mevcut davranış birebir korunur; Faz 7'deki
characterization tests bunun garantisidir.

**Stack:** `tokio` + `axum` (HTTP/SSE), `windows-rs` (registry + firewall COM), `serde` (JSON).
Cross-platform genişlerse `cross-rs`/musl toolchain ile ARM cross-compile.

**Sıralama:** önce state machine (`enum`), sonra downloader mantığı.

## Kapsam / Dosyalar

- Yeni Rust crates: `tokio` + `axum` (HTTP/SSE), `windows-rs`, `serde`
- `rgsx_manager.py` → Rust binary (10a); `qbittorrent_backend.py` → `librqbit` (10b)
- Python ↔ Rust köprüsü: JSON-RPC veya local HTTP
- Mevcut Python `manager_launcher.py` / `watchdog.py` ile sözleşme korunur

## Doğrulama

- Faz 7 characterization testleri (`tests/test_api_contract.py` vb.) Rust binary üzerinde de
  birebir geçer (sözleşme: `/api/*` + `/api/events` SSE davranışı değişmez).
- Linux/Batocera tarafında Python yolu korunur, mevcut suite baseline'ı değişmez.
- Dev makinesinde canlı: Windows'ta Rust manager spawn + tray + autostart + firewall COM.
- 10b için Linux/ARM test imkânı olmadan başlanmaz.

---

## Alt-görevler

- **TASK-002a** — manager-core state machine tasarımı → ✅ done (`tasks/done/`)
- **TASK-002b** — HTTP köprüsü: axum `/api/*` + SSE sözleşmesi → ✅ done (`tasks/done/`)
- **TASK-002c** — qbittorrent_backend subprocess köprüsü + manager-bin entegrasyonu → ✅ done (`tasks/done/`)
- **TASK-002d** — manager-windows: tray / autostart / firewall → ✅ done (`tasks/done/`)
- **Faz 10b (`librqbit`)** — 🔶 **ASKIDA:** Linux/ARM test imkânı şart olduğundan başlanmadı; Windows kapsamı
  (Faz 10a) tamamlandığı için parent görev done. Linux/ARM ortamı hazır olduğunda devam edilir.

## İlerleme

- 2026-08-11 — Roadmap'ten tasks/ yapısına taşındı (todo; henüz kodda Rust yok — `*.rs` /
  `Cargo.toml` bulunmuyor).
- 2026-08-11 — Workspace iskeleti kuruldu (5 crate, scaffold commit `dc3aa21`);
  TASK-002a (state machine) alt-görevine bölündü.
- 2026-08-12 — TASK-002a done; alt-görevler b/c/d tanımlandı; Windows kapsamına alındı
  (tray/autostart/firewall dahil; yalnızca 10b askıda). Sıradaki: TASK-002b.
- 2026-08-12 — TASK-002b done: contract (52 Rust testi) + canlı smoke OK. Sıradaki: TASK-002c.
- 2026-08-12 — TASK-002d done: tray/autostart/firewall + get_app_paths bridge + 0xC0000139 fix;
  canlı smoke + 96 Rust testi OK. **TASK-002 (Windows/Faz 10a) tamamlandı → done.** Faz 10b
  (librqbit) Linux/ARM test imkânı olmadan askıda.
