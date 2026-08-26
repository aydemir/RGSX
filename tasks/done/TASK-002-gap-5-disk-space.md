# TASK-002-gap-5 — Disk Alanı Ön-Kontrolü (Rust'ta eksik)

- **id:** TASK-002-gap-5
- **title:** Disk alanı ön-kontrolü (InsufficientDiskSpaceError)
- **status:** done
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** disk, download, guard
- **parent:** TASK-002

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `D5`, `H8`
- `ports/RGSX/network/queue.py`:
  - `D5`: `_ensure_sufficient_disk_space(dest_dir, expected_size_before_start)` → `InsufficientDiskSpaceError`
  - `H8`: `_ensure_sufficient_disk_space(dest_dir, announced_total_size)` (HTTP stream öncesi)
- `ports/RGSX/network/helpers.py`: `_ensure_sufficient_disk_space`, `InsufficientDiskSpaceError`

## Açıklama

Python, indirmeye başlamadan önce hedef diskte yeterli alan olup olmadığını kontrol eder
(hem torrent başında hem HTTP stream öncesi). Yetersizse `InsufficientDiskSpaceError`
yakalanır ve indirme temiz şekilde başarısız olur. Rust `LibrqbitEngine`/`download_torrent_source`
**disk alanı kontrolü yapmaz**; librqbit boş alanda yazma hatasına kadar gider.

## Kapsam / Dosyalar

- `manager-rs/manager-torrent/src/lib.rs` — `download_torrent`/`download_torrent_source` başına disk kontrolü
- `manager-rs/manager-bin/src/` — `/api/download` sözleşmesine `expected_size` alanı

## Doğrulama

- Yetersiz disk: Rust daemon `InsufficientDiskSpace` benzeri hata döner, partial dosya bırakmaz.
- Yeterli disk: mevcut akış değişmeden devam eder.
- Hem torrent hem HTTP (gelecekteki gap-4) yolu için kontrol uygulanır.

---

## Parite Denetimi 2026-08-15 — Ek Maddeler

### Madde A: Disk alanı ön-kontrolü hâlâ YÜKSELTİLMİYOR (❌)

- Python: `ports/RGSX/network/helpers.py:301` `_ensure_sufficient_disk_space` → `InsufficientDiskSpaceError`;
  çağrı `queue.py:786`/`832`/`1471`, yakalama `queue.py:1618`.
- Rust: `manager-download/src/http/mod.rs:56` `DownloadError::InsufficientDiskSpace` **yalnızca tanımlı**,
  hiçbir yerde `.raise()`/dönüştürme yapılmıyor; `manager-scan/src/disk.rs:30` yalnız scan içindir.
- Sonuç: Rust disk dolana kadar yazar, kullanıcıya net hata vermez (sessiz hata yutma riski).
- Bağımlılık: yok. Bu TASK'ın asıl kapsamıyla aynı kod noktası (download başı).

### Madde B: Permission (yazma izni) açık ön-kontrolü eksik (⚠️ KISMİ)

- Python: `queue.py:775-776` `os.access(dest, os.W_OK)` → `PermissionError` ön-kontrolü.
- Rust: `mod.rs:72-76` genel `From<std::io::Error>` sarmalama var; yazma izni için açık pre-check YOK.
- Bu TASK'a FOLD edildi (aynı disk-guard kod noktası). BELİRSİZ: pre-check `os.access` gibi mi
  yapılacak, yoksa ilk yazma hatası yakalanıp `Client` hatasına mı düşürülecek?
