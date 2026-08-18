# TASK-008 — Dağıtılan manager-bin'in indirme yolunun saf-Rust olduğunu doğrulama

- **id:** TASK-008
- **title:** Torrent (ve diğer) indirmelerin dağıtılan binary'de Python'a düşmediğinin kanıtı
- **status:** done
- **priority:** P2
- **created:** 2026-08-18
- **environment:** linux
- **tags:** rust, download, python-free, verification, librqbit

## Kaynak

- `d97de92` sonrası "çözüm tam Rust mı?" sorusunun kapanmayan runtime kanıtı. Statik inceleme + canlı izleme ile doğrulandı.

## Açıklama

Dağıtılan `/test/roms/ports/RGSX/manager-bin` indirme yolunun (özellikle `rgsx+torrent://`) saf Rust olduğunu teyit et: Python süreci spawn edilmiyor, katalog proxy'sine düşmüyor.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/api.rs:429` — `intercept_locally = is_torrent_url(direct_url) && state.bridge.is_some()`. Torrent URL'leri ve DDL `RGSX_NATIVE_DOWNLOAD` (varsayılan açık) ile Rust-native yola düşer; `catalog` (Python) dalı yalnız `RGSX_PYTHON_MANAGER_URL` set ise doludur.
- `manager-rs/manager-bin/src/main.rs:54` — sadece `python` alt-komutu (opsiyonel legacy bridge) `python` süreci başlatır; varsayılan çalıştırmada kullanılmaz.
- Çalışan süreç: `pgrep -P <manager-bin_pid>` ile çocuk taraması.

## Doğrulama (runtime)

1. **Env:** Çalışan `manager-bin` (PID 18588) ortamında `RGSX_PYTHON_MANAGER_URL` SET DEĞİL → `state.catalog = Some(NativeCatalog)` (saf-Rust native), `PythonCatalog` dalı hiç oluşmaz; tüm Python proxy dalları runtime'da devre dışı.
2. **Canlı indirme:** `POST /api/download` (rgsx+torrent, seçili torrent içeriği) tetiklendi; 12 sn boyunca `pgrep -aP <pid> | grep -i python` izlendi → **sıfır python çocuk süreci**. `manager-bin`'in hiç çocuk process'i yok (yalnız thread).
3. **Log:** İndirme yalnız `librqbit::session` / `manager_torrent: torrent indirildi` satırlarıyla Rust-native tamamlandı. Tek "proxy" kelimesi `GET /api/health ... qbittorrent proxy` statik log metnidir, gerçek spawn/bağlantı değildir.
4. **Yanıt:** `/api/download` Rust handler'dan `{"queued":true,"success":true}` döndü (Python proxy değil).

## Sonuç

Torrent indirme yolu dağıtılan binary'de %100 saf-Rust (gömülü librqbit); Python süreci spawn edilmiyor, catalog proxy'sine düşürülmüyor.

---
## İlerleme

- 2026-08-18 — görev oluşturuldu.
- 2026-08-18 — statik + canlı kanıt tamamlandı: env yok, çocuk python yok, log saf librqbit.
