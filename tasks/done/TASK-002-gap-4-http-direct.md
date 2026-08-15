# TASK-002-gap-4 — HTTP-Doğrudan İndirme Alt Ağacı (Rust'ta eksik)

- **id:** TASK-002-gap-4
- **title:** HTTP-direct download path (vimm/archive.org/lolroms/1fichier, header variantları, browser-challenge, 429 backoff, Range resume, arşiv imza kontrolleri)
- **status:** completed
- **priority:** P0
- **created:** 2026-08-14
- **environment:** both
- **tags:** http, download, vimm, archive.org, lolroms, 1fichier
- **parent:** TASK-002

## Faz İlerlemesi (Gap-4)

Mimari karar (onaylı): yeni `manager-http-dl` crate AÇILMADI; HTTP motoru
`manager-download` içindeki `src/http/` modüler yapısına genişletildi.

- **4a ✅ TAMAM**: stream çekirdek + guards + `native_ddl_download` bağlandı.
  - `src/http/mod.rs` (`HttpDownloader`, `DownloadRequest`, `DownloadError`, `default_browser_headers`,
    `download_async`/`download_blocking`). Challenge sırası: 403/429/503 gövde okunup
    `BrowserChallenge`/`Http` ayrımı; 200/206 stream; arşiv guards (HTML→`HtmlInsteadOfPayload`,
    imza→`InvalidArchive`, kısmi→`PartialArchiveRejected`).
  - `src/http/stream.rs` (`.part` yazma, Range resume, `CancelFlag`, progress, `finalize_part`).
    **Kritik düzeltme:** `OpenOptions` eksik `.write(true)` → dosya açılamıyordu (tüm entegrasyon
    testleri `.part=0` ile kırık gidiyordu).
  - `src/http/guards.rs` (challenge/HTML/arşiv imza + parse_content_range_total).
  - `manager-http/src/api.rs:1077` `native_ddl_download` → `HttpDownloader`'a bağlandı
    (bellek içi `bytes()` kalktı; progress callback eklendi).
  - Doğrulama: `manager-download` 17 lib + 8 integration test yeşil; `manager-http` 105 contract
    (+1 doc) korundu (native dal yalnız `RGSX_NATIVE_DOWNLOAD` açıkken devreye girer).
- **4b ✅ TAMAM**: retry/backoff `download_async` içine döngü olarak bağlandı.
  - `HttpDownloader` alanları: `max_retries` (default 5), `base_backoff` (default 5s) +
    `with_retry(max_retries, base_backoff)` builder'ı. `retry_after_wait` artık `base`
    parametreli (headers.rs).
  - Döngü sırası: 401→Http; 429→Retry-After/exp backoff + retry; 5xx→2s transient retry;
    403→challenge tespiti (BrowserChallenge) yoksa sonraki header variant'a geç; header
    variant listesi provider'a göre (`archive.org`→2 varyant, `vimm`→Connection:close çifti,
    diğer→tek). 200/206→stream+guards+finalize.
  - Testler: `rate_limit_returns_http_error` (Retry-After:0 + with_retry(3,10ms) → Http/429),
    `rate_limit_retries_then_succeeds` (429×2→200 başarı), `archive_org_tries_header_variants_on_403`
    (403→2. varyant 200). manager-download 17 lib + 10 integration = 27 test yeşil;
    manager-http 105 contract korundu.
- **4c ✅ TAMAM**: vimm.net çözümü `src/http/vimm.rs`'te gerçek (regex) implementasyon.
  - `extract_vimm_download_info(html, page_url)`: `dl_form` formundan `action` + `mediaId`
    (2 sözdizimi sırası + JS `let media=[{"ID":..}]` fallback), `dl_size`/`ZippedText` boyut
    ipucu; `url::Url::join` ile mutlak indirme URL'si. Rust `regex` look-ahead/backref
    desteklemediği için `id="dl_form"` eşleşmesi `["']dl_form["']` ile yazıldı.
  - `fetch_vimm_download_info(client, url)` (GET+parse), `fetch_vimm_file_size` (HEAD +
    Content-Disposition/Content-Length). Minimal `html_unescape` + `parse_size_to_bytes`.
  - `download_async` içine provider çözümü bağlandı: URL `vimm.net` içeriyorsa sayfa GET edilip
    gerçek indirme URL'si + referer çözülür (retry döngüsü öncesi, bir kez). `regex`+`url`
    workspace'e eklendi.
  - Test: vimm unit (form/media_id/url kurulumu, value-before-name, JS fallback, no-form→None)
    + integration `vimm_page_resolves_and_downloads` (mock + `resolve("vimm.net",addr)`).
    manager-download 21 lib + 11 integration = 32 test yeşil; manager-http 105 contract korundu.
- **4d ✅ TAMAM**: archive.org çözümü `src/http/archive_org.rs`'te gerçek.
  - `split_archive_org_path` (id/archive_name/inner_path), `normalize_archive_org_download_path`,
    `build_view_archive_url` (Python `safe="/@:$&'()*+,;=-._~"` ile uyumlu `SAFE_SLASH`
    AsciiSet — `/` ve yazım karakterleri encode edilmez), `fetch_archive_metadata`
    (`/metadata/{id}` GET → server/dir/is_dark/files, `serde_json`), `build_alt_urls`,
    `load_archive_org_cookie` (`RGSX_ARCHIVE_ORG_COOKIE_PATH` dosyası, "Cookie:" soyulur).
    URL şeması `url_origin` ile türetilir (test http / üretim https).
  - `download_async` içine bağlandı: `archive.org/download/` URL'leri için metadata çekilip
    `view_archive.php` alt-URL'i üretilir; 403 (challenge değil) → header variant → alt-URL
    fallback sırası. `DownloadRequest.cookie` alanı eklendi (api.rs:1077 de set edildi).
  - Test: archive_org unit (split/normalize/view_url/alt_urls) + integration
    `archive_org_alt_url_fallback_on_403` (mock metadata JSON + ana 403 → view_url 200,
    `resolve("archive.org",addr)` ile). manager-download 25 lib + 12 integration = 37 test
    yeşil; manager-http 105 contract korundu.

- **4e ✅ TAMAM**: WebUI→native DDL delegasyonu zaten bağlı. `api.rs:391` `RGSX_NATIVE_DOWNLOAD=1`
  ise `direct_url` `DownloadManager::resolve` ile çözülür ve `native_ddl_download`'a
  yönlendirilir; bu fonksiyon (4a'da) artık `HttpDownloader`'ı kullanır. Flag kapalıyken
  mevcut Python proxy korunur → contract baseline (105) bozulmaz.

- **4f ✅ TAMAM**: lolroms reqwest fallback `manager-download/src/http/lolroms.rs`'te.
  - `is_lolroms_url`, `normalize_lolroms_url` (lolroms SAFE set ile percent-encode,
    idempotent), `parent_url`, `lolroms_headers(referer)` (browser UA + Accept +
    Accept-Language + Referer).
  - `download_async` içine lolroms dalı: URL `lolroms.com` ise normalize edilir, **önce
    parent sayfa GET edilir** (cookie jar ısınması, Referer `https://lolroms.com/`),
    sonra dosya `Referer: parent_url` ile indirilir. Mevcut retry/stream/guards hattı
    kullanılır (yeni motor yazılmadı).
  - Workspace `reqwest` features'a `cookies` eklendi; `HttpDownloader::client()` varsayılan
    istemci artık `cookie_store(true)` (Python `requests.Session` eşleniği — parent fetch
    cookie'yi ısıtır).
  - Post-download guard'ları uygulanır: HTML/challenge (`HtmlInsteadOfPayload`) + arşiv
    imza (`InvalidArchive`) — `guards.rs`'te zaten mevcut.
  - Testler: lolroms unit (is_url normalize parent headers) + integration
    `lolroms_parent_warms_then_downloads` (mock parent GET → dosya Referer ile iner, ≥2
    istek) + `lolroms_html_guard_rejects_parentless` (HTML dönüşü guard reddeder).
    manager-download 27 lib + 14 integration = 41 test yeşil; manager-http 105 contract
    korundu. **Not:** Python external-tool (curl/wget subprocess, resume/partial-accept
    detayları) bilinçli olarak out-of-scope; reqwest fallback ile parity sağlandı.
- **(kapatıldı) 4e/4f çelişkili ⏳ notları**: 4e (WebUI→native DDL delegasyonu) zaten ✅;
  4f yukarıda ✅. External-tool (curl/wget) ayrı görev olarak out-of-scope.

## Kaynak

- `docs/PYTHON_WORKFLOW.md` düğümleri: `H0` .. `H12` (tüm HTTP-alt ağacı)
- `ports/RGSX/network/queue.py` `download_rom` HTTP dalı (satır 889–1527):
  - vimm.net: `_fetch_vimm_download_info`, `_get_vimm_file_size`, gerçek dosya adı çözümü
  - mevcut dosya kontrolü + boyut karşılaştırması (incomplete sil / already-present)
  - aynı taban farklı uzantı kontrolü
  - lolroms external tool (`_download_lolroms_with_external_tool`) + requests fallback
  - archive.org: cookie/metadata/alt-URL hazırlığı (`_try_archive_org_alternate_urls`)
  - HTTP retry döngüsü: header variantları, 401/403, **429 rate-limit backoff**,
    browser-challenge tespiti, timeout/connection retry
  - content-type HTML kontrolü (vimm), arşiv imza guards (`.7z/.zip/.rar`),
    kısmi arşiv kabul (`_should_accept_partial_archive`)
  - `_stream_response_to_path` (Range resume), `InsufficientDiskSpaceError`
- `ports/RGSX/network/one_fichier.py` — `download_from_1fichier` (ayrı modül, Rust'ta yok) → **`tasks/gap/TASK-002-gap-11-1fichier-provider.md`** (ayrı görev: provider zinciri 1F→AD→DL→RD→TB→FREE)

## Açıklama

Rust daemon'ı (`LibrqbitEngine` + `rust_daemon.download_torrent`) yalnızca **torrent**
(`source_url` magnet/`.torrent`) indirir. HTTP-doğrudan indirmeler (vimm.net, archive.org,
lolroms, 1fichier) Rust'ta **hiçbir karşılığa sahip değil**. Bu, tüm doğrudan-HTTP kaynak
indirmelerinin Python'da kalması gerektiği anlamına gelir; Rust refaktörü tamamlandığında
bu alt ağaç ya Rust'a taşınmalı ya da Python orkestratörü korunmalı. En büyük ve en nadir
dal içerikli düğüm kümesidir (browser-challenge, 429 backoff, archive alt-URL, imza guards).

## Kapsam / Dosyalar

- `manager-rs/manager-http/` veya yeni `manager-http-dl` crate — HTTP stream indirici
- `manager-rs/manager-torrent/src/lib.rs` — `download_torrent` sözleşmesine HTTP kaynak desteği
- `rust_daemon.py` — `_post_json("/api/download", {url})` zaten var; arşiv sonrası kontroller eklenmeli

## Doğrulama

- vimm.net: form/mediaId akışı + gerçek dosya adı çözümü Rust'ta çalışır.
- archive.org: cookie/metadata/alt-URL fallback zinciri parity.
- 429 rate-limit → Retry-After / exp backoff tekrar deneme.
- Browser-challenge tespitinde insan etkileşimi gerektiği doğru şekilde raise edilir.
- İndirilen `.7z/.zip/.rar` için HTML/challenge/imza guards + kısmi kabul kuralı birebir.
- Range resume: yarım dosya üzerinden devam (qBittorrent dışı HTTP için).
