# TASK-008 — Dağıtılan manager-bin'in indirme yolunun saf-Rust olduğunu doğrulama

- **id:** TASK-008
- **title:** Torrent (ve diğer) indirmelerin dağıtılan binary'de Python'a düşmediğinin kanıtı
- **status:** todo
- **priority:** P2
- **created:** 2026-08-18
- **environment:** linux
- **tags:** rust, download, python-free, verification, librqbit

## Kaynak

- `d97de92` sonrası kullanıcıya "çözüm tam Rust ile mi?" sorusu soruldu; yanıt "evet, tamamen Rust (gömülü librqbit)" verildi. Ancak dağıtılan `manager-bin`'in **çalışma anında** hiçbir Python sürecine/ proxy'sine düşmediği ayrıca doğrulanmadı (process tree / ağ bağlantısı taranmadı).

## Açıklama

Dağıtılan `/test/roms/ports/RGSX/manager-bin` indirme yolunun (özellikle `rgsx+torrent://`) saf Rust olduğunu teyit et: Python süreci spawn edilmiyor, katalog proxy'sine düşmüyor. `manager-http` içindeki `state.catalog` kontrolü ve `intercept_locally` dalı incelenir.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs` — `/api/download` handler, `intercept_locally`, `state.catalog` proxy dalı.
- Çalışan süreç: `pgrep`/`/proc` ile Python süreç taraması.
- Log: `manager-bin.log` ("proxy"/"python" kelimeleri).

## Doğrulama

- Bir torrent indirmesi sırasında `pgrep -f python` boş (indirme context'inde).
- Log'da Python/catalog proxy referansı yok; yalnız librqbit satırları.
- `/api/download` (rgsx+torrent) yanıtı Rust-native backend'ten geliyor.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu (torrent fix sonrası "saf Rust mı?" sorusunun kapanmayan kanıtı).
