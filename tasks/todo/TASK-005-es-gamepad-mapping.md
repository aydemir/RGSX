# TASK-005 — ES gamepad map'ini RGSX'e okutma (native controller mapping)

- **id:** TASK-005
- **title:** EmulationStation `es_input.cfg` (ve `SDL_GAMECONTROLLERCONFIG`) map'inin RGSX UI'a uygulanması
- **status:** in-progress
- **priority:** P1
- **created:** 2026-08-14
- **environment:** both
- **tags:** gamepad, es-input, retrobat, batocera, controller-mapping, tvui

## Kaynak

- Kullanıcı analizi (2026-08-14): RetroBat/Batocera'da `ES map (es_input.cfg) → launcher/configgen → emülatör profili` akışı; RGSX bir "port" olarak aynı kapıdan giriyor. Kullanıcıya ikinci bir "Remap Controls" dayatmak gereksiz sürtünme.
- Doğrulama (web araştırması): `es_input.cfg` yolları ve formatı teyit edildi (bkz. Açıklama).

## Açıklama

RGSX, gamepad navigasyonunu EmulationStation ile **aynı fiziksel tuşlardan** almalı; kendi içinde ayrı bir remap zorlamamalı. Yöntem: launcher'ın özel dosya üretmesini beklemek yerine, **ES'in zaten yazdığı map'i RGSX'in okuması** (+ launcher `SDL_GAMECONTROLLERCONFIG` set ettiyse onu yemesi).

### Doğrulanan gerçekler
- **Batocera:** `/userdata/system/configs/emulationstation/es_input.cfg` (+ son kullanılan pad için `es_last_input.cfg`).
- **RetroBat:** `<RetroBat>\emulationstation\.emulationstation\es_input.cfg` (gizli `.emulationstation` klasörü).
- **Format:** `<inputConfig type="joystick" deviceName="…" deviceGUID="…">` → `<input name="a" type="button" id="4" value="1" code="292" />`. Yani ES aksiyonu → SDL `code` eşlemesi hazır.
- `es_settings.cfg` (aynı klasör) `swap-a/b` gibi UI tercihlerini tutar; isteğe bağlı onurlandırılabilir.

### ES → RGSX action tablosu (taslak)
| ES aksiyonu | RGSX aksiyonu | Not |
|-------------|---------------|-----|
| `a` | confirm | menüde seç / indirme başlat |
| `b` | back | geri / vazgeç |
| `x` / `y` | secondary / context | (isteğe bağlı: arama aç, favori) |
| `start` | menu / options | ayarlar |
| `select` | toggle view | (katalog↔kuyruk) |
| `up`/`down` | nav-up / nav-down | liste gezinme (axis veya hat olabilir) |
| `left`/`right` | nav-left / nav-right | platformlar arası geçiş |
| `pageup`/`pagedown` | page-up / page-down | uzun liste sayfalama |
| `hotkey`+`start` | exit | (UI'dan çıkış, isteğe bağlı) |
| analog `up/down/left/right` | nav (yinelemeli) | klasik D-pad yerine stick |

### Path keşif sırası (startup / "Sync from ES")
1. `SDL_GAMECONTROLLERCONFIG` env var varsa → SDL gamecontroller layout (bonus, özellikle Batocera).
2. `es_last_input.cfg` → sadece son kullanılan pad (GUID eşleşirse en hızlı).
3. `es_input.cfg` → `deviceGUID` ile şu an bağlı gamepad'e eşleşen `<inputConfig>` bloğu.
4. Fallback: varsayılan Standart Gamepad eşlemesi (browser/webui için).

### Öncelik / override kuralı
- Kullanıcının `controls.json` (veya RGSX remap) override'ı **kazanır**.
- ES map'i yalnızca **ilk dolum** veya açık "Sync from ES" tetikleyicisi ile seed eder.
- **YAPMA:** RetroBat upstream'ten RGSX'e özel generator bekleme; emülatör `.rmp`/core remap okuma; ES'i runtime'da izleme.

### ⚠️ Katman kararı (implementasyon öncesi netleştirilecek)
ES→SDL-code çevirisi **yalnızca RGSX SDL code'u native tüketirse** (pygame / SDL2 / gilrs) birebir temiz çalışır.
Mevcut webui **tarayıcı Gamepad API** (standart mapping: A=0,B=1,Up=12,Down=13…) kullandığı için:
- (A) backend `es_input.cfg`'yi expose eder, webui ES-action → browser standart index'e çevirir (default SDL mapping için çalışır; custom ES remap için ek iş), **veya**
- (B) TV girdi yolu **native SDL2**'ye alınır (manager-tvui/gilrs), ES map'i SDL code üzerinden birebir uygulanır.
Implementasyona geçmeden önce (A) mı (B) mi hedeflenecek karara bağlanmalı.

## Kapsam / Dosyalar (tahmini — katmana göre netleşir)
- Okuma/parse: yeni modül (ör. `manager-tvui` veya `manager-http` altında `es_input` parser) — `es_input.cfg` XML parse + GUID eşleme.
- Backend endpoint (webui yolu A için): `GET /api/es-input` (veya `controls` seed).
- Frontend (webui): `App.vue` TV handler'ında ES-action → Gamepad index çeviri katmanı.
- VEYA native yol (B): Rust SDL2/gilrs gamepad loop + action tablosu.
- `controls.json` okuma/yazma + "Sync from ES" UI tetikleyici.

## Doğrulama
- Batocera/RetroBat'te ES'te bir pad'i yeniden map'le → RGSX aynı pad ile menüde aynı tuşlarla gezinsin (ikinci remap gerekmesin).
- `controls.json` varsa ES map'ini ezesin; "Sync from ES" tıklanınca ES değerleriyle seed'lansın.
- Path keşif sırası: env var / es_last_input / es_input / fallback her biri ayrı ayrı test edilsin.

---

## İlerleme
- 2026-08-14 — Analiz doğrulandı, plan/task olarak bırakıldı (implementasyona geçilmedi). Katman (A/B) kararı bekliyor.
- 2026-08-14 — **(A) yolu implemente edildi:** `manager-http/src/es_input.rs` (XML parse + path keşfi: `RGSX_ES_INPUT` → Batocera `/userdata/.../es_input.cfg` → `es_last_input.cfg` → `RGSX_RETROBAT_ROOT`), `GET /api/es-input` ucu, webui `App.vue` TV modunda `/api/es-input` tüketip `esMap` ile navigasyonu ES ile senkronluyor (up/down/confirm/back/pageup/down). Parser unit testi + canlı test: `found:true` (doğru `rgsx` eşlemesi) ve `found:false` (fallback) doğrulandı. **Kalan:** (B) native SDL2 girdi yolu = custom ES remap sadakati (tarayıcı Gamepad API SDL code expose etmediği için (A)'da custom remap birebir yansımaz).
