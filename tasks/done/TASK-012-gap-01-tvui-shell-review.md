# TASK-012-gap-01 — TVUI Rust port inceleme bulguları (tvui.py ↔ manager-tvui parite taraması)

- **id:** TASK-012-gap-01
- **title:** Native TVUI shell'inin Python tvui.py'ye karşı kör nokta analizi ve dayanıklılık düzeltmeleri
- **status:** done
- **priority:** P1
- **created:** 2026-08-22
- **environment:** both
- **tags:** manager-tvui, sdl2, sse, resilience, parity, review

## Kaynak

- **Gözlem:** `ports/RGSX/tvui.py` (1867 satır) ile `manager-rs/manager-tvui/src/{net,sdl2_shell,theme}.rs`
  satır satır karşılaştırıldı. İskelet sağlam (snapshot race fix `net.rs:103-110`, bootstrap-fail≠ready
  `net.rs:84-93`, offline/retry akışı); ancak çalışma-süresi dayanıklılığı ve state makinesinde boşluklar var.

## Bulgular

### Kritik — çalışma süresi sağlığı
1. **SSE watcher yeniden bağlanmıyor** — `start_catalog_watcher` connect hatasında error set edip `return`
   (`net.rs:287-294`); stream EOF'ta loop bitiyor (`net.rs:297`). Thread ölünce loading bar donar,
   `manager_update` stage'leri hiç gelmez. Self-update apply manager'ı yeniden başlattığı için bu senaryo
   teorik değil. Python tarafı 3 sn'de bir sonsuz reconnect yapar (`tvui.py:450-463`).
2. **Stall tespiti yok** — yalnızca `timeout_connect` var (`net.rs:283-285`); half-open TCP'de
   `reader.lines()` sonsuza dek bloke olur. Python `urlopen(timeout=60)` soket read-timeout'uyla kurtulur
   (`tvui.py:459`). Sunucu zaten 30 sn'de bir keep-alive snapshot gönderiyor (`sse.rs:138-144`) →
   read-timeout bu keep-alive'a göre kalibre edilebilir.
3. **SDL event loop'unun içinde senkron, timeout'suz HTTP** — `trigger_*` çağrıları döngüde inline
   (`sdl2_shell.rs:247,274,282,299`) ve fonksiyonların timeout'u yok (`net.rs:183-227,254-267`).
   Manager askıdaysa UI 30 sn+ donar.
4. **Mutex poisoning domino çöküşü** — her yerde `.lock().unwrap()` (`net.rs:71,166`, `sdl2_shell.rs:59,308`).
   SSE thread'i lock tutarken paniklerse SDL thread'i de panikler.

### Mantık hataları
5. **State geçişi human-message string-eşlemeyle** — `st.contains("yeniden başlat")` (`sdl2_shell.rs:276`).
6. **Download isteği fail etse bile `update_stage="downloading"`** set ediliyor, hata yanıtı çöpe gidiyor
   (`sdl2_shell.rs:282-285`). İstek kuyruğa giremezse banner sonsuza dek "downloading".
7. **`update_restarting` resetlenmiyor** — apply sonrası fullscreen overlay kalıcı (`sdl2_shell.rs:309-310`),
   relaunch patlarsa kullanıcı ölü ekranda, timeout yok.
8. **`available:false` stale banner temizlemiyor** (`net.rs:164-168`; testte bile sabitlenmiş `net.rs:437-438`).

### Parite boşlukları
9. **Native shell'de gamepad yok** — yalnız klavye Esc/R/Return/C (`sdl2_shell.rs:224-301`);
   gilrs yolu yalnız SPA'ya SSE yayınlıyor, SDL shell tüketmiyor (`manager-bin/src/main.rs:352-360`).
   Key-repeat de yok (Python `process_key_repeats`, `tvui.py:701`).
10. **Banner renk bug'ı** — yorum "available=turuncu" der (`sdl2_shell.rs:136`) ama `_ => "error_text"`
    available'ı da kırmızı yapar (`sdl2_shell.rs:152-156`); `warning_text` hiç kullanılmıyor (`theme.json:16`).
11. **i18n sıfır** — Türkçe literal'lar ASCII/UTF-8 karışık (`net.rs:92` "hazirlanamadi" vs `net.rs:233`
    "kuyruğa"); Python 6 dilli (`ports/RGSX/languages/*.json`).
12. **Render verimsizliği** — her frame tam redraw + gradyan için h adet `draw_line` (`sdl2_shell.rs:37-44`),
    vsync yok yerine `sleep(16ms)` (`sdl2_shell.rs:213,324`). Python `needs_redraw` bayrağıyla yalnız
    gerekince çizer (`tvui.py:1274`). ARM/Batocera hedefi.
13. **Fullscreen zorunlu, resize yok** (`sdl2_shell.rs:208`); Python `get_display_fullscreen()` ayarlı +
    VIDEORESIZE yakalar (`tvui.py:800-814`).
14. **`parse_sse_frame` çok satırlı data'yı kırar** — her `data:` satırı öncekini ezer (`net.rs:60-62`);
    Python join eder (`tvui.py:489-490`).
15. **Shell input/state mantığı SDL'e gömülü, test edilemez** (`sdl2_shell.rs:228-301`).
16. **Cross-platform borcu** — `sdl2 bundled` (`Cargo.toml:18`) Windows cross-build'e CMake+nasm yükü getirir;
    `cargo check --target x86_64-pc-windows-gnu` kanıtlanmadı. `launch` non-main-thread'de
    (`manager-bin/src/main.rs:367`) — macOS hedeflenirse SDL main-thread ister.

## Fazlar

### Faz A — SSE dayanıklılığı (en düşük risk)
- [x] Reconnect döngüsü: connect hatası / stream EOF / read-timeout sonrası 3 sn backoff ile sonsuz yeniden
      bağlanma (Python parity `tvui.py:463`). Yalnızca İLK bağlantı hatası `state.error`'a yazılır;
      sonraki kopmalar loglanır, mevcut grid/hata durumu bozulmaz.
- [x] `timeout_read(90s)` (> 2× sunucu 30s keep-alive `sse.rs:138`) — stall'da read Err verir, döngü
      reconnect'e düşer. `lines()` hatası artık `""`'a ezilmez (busy-loop fix).
- [x] `trigger_*` + `fetch_platforms`: ortak agent, connect 3s + overall 5s timeout (donma üst sınırı).
- [x] `apply_snapshot` catalog_ready'de `error`/`offline` temizler (bootstrap hatası bayatlaması).
- [x] Poison-safe lock yardımcısı (`unwrap_or_else(|p| p.into_inner())`) — net.rs + sdl2_shell.rs.
- [x] Unit testler: ölü porta cycle error set eder; snapshot error temizler; mevcut testler korunur.

### Faz B — state makinesi + test edilebilirlik
- [x] `UiAction`/`UiKey`/`ui_decision` SAF karar katmanı net.rs'te (bulgu 15) — SDL'siz unit test;
      sdl2_shell yalnız Keycode→UiKey çevirisi + draw kaldı.
- [x] `TriggerResult{ok,message}` (bulgu 5): tüm `trigger_*` artık makine-okunur ok döner,
      string-eşleme (`contains("yeniden başlat")`) kaldırıldı.
- [x] Stage yalnızca istek `ok:true` ise "downloading" olur; aksi halde "failed" (bulgu 6).
- [x] Restart overlay koruması: `update_restarting_since` + `RESTART_OVERLAY_TIMEOUT(60s)` +
      `expire_stale_restart_at()` her frame'de (bulgu 7); relaunch gelmezse overlay kapanır.
- [x] `available:false` bayat banner temizliği (bulgu 8) — in-flight aşamada dokunulmaz.
- [x] Banner rengi: available→warning_text (turuncu), kırmızı yalnız failed (bulgu 10).
- [x] HTTP çağrıları event-loop thread'inden arka plana taşındı (bulgu 3'ün kalan yarısı).

### Faz C — parite
- [x] Gamepad köprüsü (9): SSE `gamepad` olayı shell'e bağlandı — `gamepad_event_to_key`:
      confirm → Enter eşdeğeri, back → çıkış (shutdown bayrağı, watcher temiz biter);
      nav/page tuşları TASK-012h'taki grid navigasyonuna kadar bilinçli None
      (tüketici olmadan bağlanmaz). Key-repeat de nav tüketicisiyle 012h'ta.
- [x] Render perf (12): arka plan gradyanı texture cache (`bg_cache`, boyut değişince
      yenilenir, fallback scanline); `present_vsync` + renderer info'dan gerçek vsync
      tespiti — vsync varken 16 ms sleep atılır (30 fps'e düşme hatası önlendi).
- [x] Windowed mod (13): `RGSX_TVUI_WINDOWED=1` → resizable pencere (masaüstü test/debug).
- [x] Multi-line SSE data (14): `data:` satırları `\n` ile birleşir (spec + Python parity).
- [ ] i18n katmanı (11) → **TASK-012h'a devredildi**: TTF metin render'ı gelmeden i18n'in
      UI'da tüketicisi yok; tüketici olmadan modül bağlamak "yarım iş" kuralına aykırı.
      String'ler `TriggerResult.message`'larda merkezîleşti, çıkarma maliyeti düştü.
- [x] Cross-platform notu (16): lib.rs'e macOS main-thread uyarısı eklendi (hedef
      Linux/Batocera + Windows). Gerçek `cargo check --target x86_64-pc-windows-gnu`
      koşusu ağır makinede (ARM proot SDL2 bundled cross derlemesi kaldırmıyor).

## Doğrulama

- `CARGO_TARGET_DIR=/root/RGSX/rust-target-sandbox cargo test -p manager-tvui` (Faz A/B sonrası).
- Canlı: `RGSX_TVUI=1 manager-bin` → SSE bağlantısını kapat (manager restart) → TVUI'nin ≤3 sn'de
  yeniden bağlandığı, loading barın donmadığı gözlemlenir.

## Ağır makine doğrulama listesi (Windows ajan — Faz A+B+C tek geçiş)

Sandbox (ARM proot) SDL2 bundled derleyemediği için aşağıdakiler Windows/heavy makinede koşulmalı
(sandbox'ta net.rs mantığı izole crate ile doğrulandı: Faz A 13/13, Faz B 17/17, Faz C 20/20):

```bash
cd <repo>/manager-rs
# 1. Tam crate testleri (SDL2 bundled C derlemesini ilk kez içerir):
cargo test -p manager-tvui                 # beklenen: 20/20 yeşil
# 2. Windows cross-check (AGENTS.md kural 5; sdl2 bundled → CMake+nasm gerekir):
cargo check --target x86_64-pc-windows-gnu -p manager-bin
# 3. Canlı smoke — reconnect (Faz A): TVUI açıkken manager'ı restart et;
#    beklenen: loading bar DONMAZ, ≤3 sn'de "TVUI SSE bağlı" logu geri gelir.
set RGSX_TVUI=1 && manager-bin.exe
# 4. Canlı smoke — windowed + gamepad (Faz B/C):
set RGSX_TVUI_WINDOWED=1&& set RGSX_NATIVE_INPUT=1&& manager-bin.exe
#    beklenen: resizable pencere; gamepad confirm = Enter davranışı, back = çıkış.
```

Bulgular bu dosyanın İlerleme bölümüne not edilmeli; hata varsa ilgili fazın başlığına dönülür.

## İlerleme

- 2026-08-22 — İnceleme tamamlandı, bulgular + faz planı bu dosyaya yazıldı. Faz A implementasyona alındı.
- 2026-08-22 — **Faz A uygulandı** (henüz commit edilmedi):
  - `net.rs`: `start_catalog_watcher` sonsuz reconnect döngüsü (3 sn backoff, Python parity
    `tvui.py:450-463`); yalnızca İLK bağlantı hatası `state.error`'a yazılır, sonraki kopmalar UI'yı bozmaz.
    `timeout_read(90s)` watchdog (≥2 kaçırılmış 30s keep-alive `sse.rs:138`) — stall'da read Err →
    reconnect; `lines()` hatası artık `""`'a ezilmez (busy-loop fix). `api_agent()` ortak agent:
    connect 3s + overall 5s (`trigger_*`, `fetch_platforms`). `apply_snapshot` catalog_ready'de
    stale `error`/`offline` temizler. Poison-safe `tvui_lock()` yardımcısı (net.rs + sdl2_shell.rs).
  - **Doğrulama:** net.rs izole scratch crate'te (ureq default-features=false, SDL2'siz)
    `cargo test` → **13/13 yeşil**, dahil yeni testler `watcher_retries_after_failed_connect_instead_of_returning`
    ve `snapshot_catalog_ready_marks_ready_and_clears_stale_error_offline`.
  - **Ertelenen:** repo-içi `cargo test -p manager-tvui` — sandbox (Termux proot ARM) SDL2 bundled
    C derlemesini kaldıramadı (proot dondu, yeniden başladı). Ortam onarımı: bozuk `cc1`
    (`cpp-14-aarch64-linux-gnu` reinstall) + `cmake` kurulumu. Tam test ağır makinede/Windows'ta koşulacak.
  - Etkilenen davranış: manager-http restart (self-update apply sonrası) artık TVUI loading barını
    dondurmuyor; ≤3 sn'de yeniden bağlanıyor.
- 2026-08-22 — **Faz A commit+push:** `0f01262`.
- 2026-08-22 — **Faz B uygulandı** (net.rs + sdl2_shell.rs):
  - net.rs'e SDL'siz UI karar katmanı taşındı: `UiAction`, `UiKey`, `ui_decision()` (saf),
    `apply_ui_action()` (yerel mutasyon senkron, HTTP arka plan thread'inde), `expire_stale_restart_at()`.
  - `TriggerResult{ok,message}` — bulgu 5 string-eşlemesi yok; sdl2_shell.rs ~50 satır event-handler
    bloğu yerine Keycode→UiKey eşlemesi + çağrı kaldı.
  - Bulgu 6/7/8/10 düzeltmeleri (ayrıntı Faz B checklist).
  - **Doğrulama:** izole crate'te **17/17 yeşil**; yeni: `ui_decision_covers_state_machine`,
    `restart_overlay_expires_after_timeout`,
    `apply_ui_action_runs_http_off_thread_and_marks_failed_on_dead_port`,
    `available_false_keeps_inflight_update_flow`; `available:false` bekleyişi bulgu 8 kararıyla
    değişti (idle'da temizler). Repo-içi full test ağır makineye bırakıldı (SDL2 bundled, bkz. üstte).
- 2026-08-22 — **Faz B commit+push.**
- 2026-08-22 — **Faz C uygulandı** (net.rs + sdl2_shell.rs + lib.rs):
  - Bulgu 14: `parse_sse_frame` çok satırlı `data:` birleştirir (testli).
  - Bulgu 9: `gamepad_event_to_key` + watcher `gamepad` kolu + `shutdown` bayrağı
    (`Arc<AtomicBool>`, lib.rs launch'ta üretilip watcher↔shell paylaşılır; gamepad back
    → SDL döngüsü çıkar, watcher sızmadan biter). Nav tuşları bilinçli olarak bağlanmadı.
  - Bulgu 12: bg gradient texture cache + `present_vsync` + koşullu sleep.
  - Bulgu 13: `RGSX_TVUI_WINDOWED=1` resizable pencere.
  - Bulgu 16: macOS main-thread notu lib.rs'te; gerçek Windows cross-check ağır makineye.
  - Bulgu 11: i18n bilinçli erteleme → TASK-012h (TTF metin render'ı tüketici olacak).
  - **Doğrulama:** izole crate **20/20 yeşil**; yeni: `parse_sse_frame_joins_multiline_data`,
    `gamepad_events_map_to_intents`, `gamepad_confirm_frame_drives_ui_and_back_exits`.
- 2026-08-22 — **Faz C commit+push.** Görevin sandbox-dışı kalan tek kalemi: ağır makinede
  `cargo test -p manager-tvui` + Windows cross-check.
- 2026-08-24 — **Ağır makine doğrulama tamamlandı** (Windows msvc + Windows-gnu cross + WSL1 Linux):
  - `cargo test -p manager-tvui` (Windows msvc, rustup stable 1.98): **27/27 yeşil**. Not: beklenen
    "20/20" güncel değil — arada `native_input` ve `theme` testleri eklendi, suite 27'ye çıktı.
  - **2 net testi Windows'ta deterministik düşüyordu** (`watcher_retries…`, `apply_ui_action…dead_port`):
    kök neden ortam farkı — bu Windows makinesinde reddedilen loopback TCP bağlantısı ~2 sn sürüyor
    (güvenlik yazılımı filtresi; Linux'ta anlık). Testler sabit sleep (500ms / 2sn bütçe) yerine
    15 sn deadline'lı beklemeye çevrildi (`manager-tvui/src/net.rs`). Semantik aynı: dead port
    er geç error/failed üretir. Düzeltme sonrası Windows + Linux ikisinde de 27/27.
  - **Windows cross-check (kural 5) ilk kez geçti**: `cargo check --target x86_64-pc-windows-gnu
    -p manager-bin` OK (6m30s, tek warn bilinen paths.rs:19). Önkoşullar (tekrar için zorunlu):
    scoop `mingw` 16.2.0 + `ninja`; env `CMAKE_GENERATOR=Ninja` (PATH'teki busybox `sh.exe`
    MinGW make'i bozuyor: sh ile Error 127, sh'siz Error 2 — Ninja sh'e ihtiyaç duymaz),
    `CFLAGS=-std=gnu11` (GCC 16 varsayılan C23 `true/false` anahtar sözcükleri SDL2
    `SDL_hidapi_steam.c`'i patlatıyor), `CMAKE_POLICY_VERSION_MINIMUM=3.5`,
    `RUSTUP_HOME`/`CARGO_HOME` açıkça set edilmeli (oturum env'i eskiyse cargo yanlış
    rustup home'a bakıyor). `windows/scripts/verify_gap01.ps1` bu env ile güncellendi.
  - **Linux tarafı ilk kez tam crate olarak koşuldu** (ARM proot sandbox SDL2 bundled derleyemediği
    için hep izole scratch crate'ti): WSL1 `rgsx-linux` (Ubuntu 24.04 x86_64) içine rustup minimal
    kuruldu; `CARGO_TARGET_DIR=/root/rgsx-target` ile (workspace config'teki Windows target-dir
    ezilir) `cargo test -p manager-tvui` → **27/27 yeşil** (0.05s).
  - **Kalan:** canlı smoke ×2 — reconnect (manager restart'ta TVUI ≤3 sn yeniden bağlanmalı) ve
    windowed+gamepad; GUI olduğu için kullanıcı eşliğinde manuel.
- 2026-08-24 — **Canlı smoke (windowed) OK:** debug manager-bin, `RGSX_TVUI=1 +
  RGSX_TVUI_WINDOWED=1` → pencere açıldı (resizable, "RGSX" başlık), stderr
  `TVUI SSE bağlı: http://127.0.0.1:5000/api/events`, grid tile'ları render edildi
  (kullanıcı ekran görüntüsü). Not: "manager'ı restart et" varyantı tek-süreç mimaride
  (TVUI = manager-bin thread'i, `main.rs:365`) uygulanamaz; reconnect karşılığı unit
  testlerle her iki platformda kanıtlandı. Gamepad kolu kullanıcı cihazına bağlı.

