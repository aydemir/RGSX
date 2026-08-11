# Utils Paketi Referansı (`utils/`)

> Faz 6-5: eski tek parça `utils.py` modüllere bölündü. **Davranış korunur**: tüm
> fonksiyonlar/modül-state `utils.X` üzerinden aynı isimle erişilebilir; logger kimliği
> korunur (`logger = logging.getLogger("utils")`). Satır referansları commit `c5c5685` itibarıyla.

## Modül haritası

| Modül | Satır | Rol |
|---|---|---|
| `extract.py` | 1436 | Arşiv çıkarma: `extract_data`, `extract_zip/rar/7z`, özel platform handler'ları (`handle_dos/ps3/psvita/scummvm/xbox`) |
| `services.py` | 722 | Servis durumu: `restart_application`, bağlantı durumu izleme, web/DNS boot toggle, VersionClean |
| `games.py` | 631 | `load_sources`, `load_games`, platform oyun sayısı cache (kalıcı) |
| `media.py` | 527 | Platform görselleri, source rozetleri (SVG→PNG), müzik (`play_random_music`, mixer) |
| `torrent.py` | 444 | Torrent manifest: bencode, cache, `build_torrent_download_url`, refresh isteği bayrağı |
| `extensions.py` | 317 | `load_extensions_json` (es_systems.cfg'dan tek seferlik üretim + cache), `check_extension_before_download` |
| `api_keys.py` | 300 | 1Fichier/Alldebrid/Debridlink/RealDebrid/TorBox anahtar yönetimi + `provider_keys_status` |
| `files.py` | 287 | Disk kullanımı, dosya arama, klasör çözümleme, platform adı normalizasyonu |
| `text.py` | 205 | `sanitize_filename`, `get_clean_display_name`, `wrap_text`, boyut biçimleme |
| `history_matches.py` | 196 | History yerel eşleşme araması + "eşleşme yok" log soğutması |
| `security.py` | 167 | `_redact_settings_file_text`, `redact_sensitive_settings`, `generate_support_zip` |
| `sorting.py` | 132 | Oyun listesi sıralama + `parse_game_size_to_bytes` |
| `__init__.py` | 308 | Re-export yüzeyi (13 modülden ~135 sembol) |

## Re-export disiplini (`__init__.py`)

- `from utils.<mod> import (...)` + açık `__all__` (172-308).
- **Logger temizliği**: `urllib3` ve `requests` seviyeleri WARNING'e çekilir (gürültü engeli).
- Kamu API'si değişmedi: tüketiciler hâlâ `utils.load_sources`, `utils.sanitize_filename`,
  `utils.check_extension_before_download` vb. çağırır.

## Kritik fonksiyonlar

### `load_sources` (games.py:341) — 15 çağıran
Platform listesini yükler; torrent manifest refresh bayrağı `request_torrent_manifest_refresh()`
ile işaretlenebilir (WebUI `/api/update-cache` sonrası kullanılır). WebUI `cache.py` içinde
ETag/Last-Modified önbelleği ile süslenir.

### `check_extension_before_download` (extensions.py:220) — 12 çağıran
`(url, platform, game_name, is_zip_non_supported)` tuple'ı döner; hata → `None` (çağıran 400 verir).
Sıralama: BIOS/ZIP otomatik çıkarma → PS Vita ZIP çıkarmaz → DOS ZIP/RAR çıkarma zorla →
desteklenen uzantı → arşiv (bilinmeyen listede olsa bile çıkarılır) → bilinmeyen uzantı
(allow_unknown'a bağlı). `is_extension_supported` platformu `config.platform_dicts[].platform_name`
üzerinden `ROMS_FOLDER/<folder>` dizinine eşler.

### `load_extensions_json` (extensions.py:55)
İlk çağrıda `es_systems.cfg`'den `rom_extensions.json` üretir (RetroBat > Batocera önceliği,
çoklu cfg birleştirme, `_extensions_cache` ile bir kez). Cache boşsa indirmeyi engellemez,
"unknown" muamelesi yapar.

### `restart_application` (services.py:55) — 6 çağıran
Uygulamayı yeniden başlatır; WebUI `/api/restart` 2 sn gecikmeli thread'den çağırır.

### Torrent manifest (torrent.py)
- `is_torrent_manifest_url` / `build_torrent_download_url` — `.torrent` URL'lerini indirilebilir
  dosya URL'lerine çevirir (`_TORRENT_DOWNLOAD_SCHEME`).
- `request_torrent_manifest_refresh` / `is_torrent_manifest_refresh_requested` —
  uygulama geneli refresh bayrağı (tek tüketici tüketir).
- Kalıcı manifest cache (`_save_persistent_torrent_manifest_cache`) + lock.

### Güvenlik (security.py)
- `redact_sensitive_settings` — hassas anahtarları `_REDACTED_PLACEHOLDER` ile değiştirir
  (`_SENSITIVE_SETTING_KEY_RE`).
- `_redact_settings_file_text` — support ZIP'e konacak `rgsx_settings.json` metnini temizler.
- `generate_support_zip` — destek paketi üretimi (WebUI `/api/support`).

## Eklenti notu

- `utils.extract` `_handle_special_platforms` + `unavailable_systems` — platforma özel çıkarma
  kuralları.
- `utils.media` `_platform_image_folders` + `get_platform_image_payload` — WebUI `/api/image/<platform>`.
- `.coveragerc` `omit`: `*/utils/*` test kapsamı dışında (beklenen).

## İlgili dosyalar

- `utils/__init__.py` — re-export + `__all__`
- `utils/games.py` — `load_sources`, `load_games`
- `utils/extensions.py` — uzantı kontrolü/üretimi
- `utils/services.py` — restart + bağlantı durumu
- `utils/torrent.py` — torrent manifest
- `utils/security.py` — redaksiyon + support ZIP
