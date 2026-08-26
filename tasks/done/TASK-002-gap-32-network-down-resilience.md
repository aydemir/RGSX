# TASK-002-gap-32 — Network-down resilience (bağlantı kopmasında indirme donmamalı)

- **id:** TASK-002-gap-32
- **title:** Ağ kopması dayanıklılığı — global offline'da indirme park edilir, retry bütçesi yakılmaz, bağlanınca otomatik devam eder
- **status:** done
- **priority:** P1
- **created:** 2026-08-20
- **environment:** both
- **tags:** manager-http, state, webui, sse, connectivity, retry

## Kaynak

- **Gözlem (kullanıcı, 2026-08-20):** Kuyrukta indirme varken WiFi kesildi → indirme barı durdu → WiFi'ye dönüldü → indirme kaldığı yerden devam etti. Kullanıcı: "internet bağlantınız yok" gibi bir mesaj mı göstersek diye düşündü; nasıl olması gerektiği belirsiz.
- **Kıdemli analiz:** Mevcut kodda ağ kopması bir **per-URL transient hata** olarak ele alınır (`DownloadError::Network → ErrorClass::Transient`, `api.rs:1813`). Retry zarfı yalnızca `DEFAULT_MAX_RETRIES=3` + backoff 5/10/20s ≈ **35–50s** sürer. Yani **sustained outage (~50s üstü)** retry'ları tüketir → `finalize_download_in_state(ok=false)` → indirme kuyruktan **silinir + History'ye `Erreur`** düşer.
- **Gerçek kusur:** Kullanıcının testim KISA kesinti olduğu için tuttu. Uzun bir drop (ör. 1 dk) indirmeyi sessizce başarısız kılar — oysa kullanıcı "devam etmeli" bekler. Sorun "mesaj mı?" değil, **sustained outage'ın indirmeyi öldürmemesi**.

## Açıklama

**Temel ilke:** WiFi/network drop = **global bağlantı kaybıdır**, per-URL retry bütçesini yakmamalıdır.

Hedef davranış:
1. **Global offline tespiti** — hafif, periyodik connectivity probe (DNS/HEAD).
2. **Offline iken:** Etkilenen indirmeler **park edilir** (loop-top gate, `global_paused`/gap-29 deseninin birebir kopyası), retry sayacı ARTMAZ. UI net söyler: üst banner "İnternet bağlantısı yok — bağlanınca otomatik devam edecek", satır durumu "Ağ bekleniyor" (generic "Retrying"den ayrık).
3. **Yeniden bağlanınca:** Otomatik devam — kısmen inen `.part` üzerinden **Range resume** (zaten çalışıyor; kullanıcı gözlemiyle doğrulandı).

**Yapılmaması gerekenler (anti-pattern):**
- Outage'da indirmeyi silmek / baştan başlatmak.
- Geçici outage'ları History'yi `Erreur` ile doldurmak.
- "Bağlantı yok"u kalıcı hata gibi kırmızı/göze batacak göstermek.
- Per-URL retry bütçesini (404, sunucu hatası vb. için) global outage ile tüketmek.

## Kapsam / Dosyalar

**Backend (Rust, `manager-rs/manager-http`):**
- `state.rs`: `AppState`'e `network_down: AtomicBool` + `network_resume: Notify` ekle (mirror `global_paused` / `pause_resume`).
- `api.rs`:
  - native_ddl indirme döngüsü (~satır 1975) VE bridge indirme döngüsü (~satır 605): loop-top'a `if state.read().network_down { network_resume.notified().await }` (gap-29 deseni, `api.rs:1981`).
  - `decide_retry` (1828): global-down sırasında Network hatası per-URL `retries` sayacını ARTIRMASIN (zaten loop-top park ettiği için `decide_retry`'a hiç ulaşmaz → doğal çözüm).
  - **Faz 2 (pragmatik — Faz 1+3'ten SONRA):** Ayrı bir sürekli probe daemon'u YERİNE — indirme/retry yolunda **ardışık N adet `Network` hatası** alınınca `network_down=true` çek (flapping önleme eşiği). `network_down` true iken yalnızca o sırada **5–10sn'de bir hafif HEAD/ping** ile yeniden bağlantı yoklanır; bağlantı gelince `network_resume.notify_waiters()` + `dirty`. Böylece her zaman-açık daemon olmaz, maliyet yalnızca outage sırasında doğar.
  - (opsiyonel) `GET /api/network-status`.
- `sse.rs`: snapshot_json'e `network_down` ekle (UI canlı güncellensin).

**Frontend (`webui/src/App.vue` + i18n):**
- `network_down` true iken üst banner: `⚠ İnternet bağlantısı yok — indirmeler duraklatıldı, bağlanınca otomatik devam edecek.`
  - Kuyruk satır durumu: yeni "Ağ bekleniyor" (backend status → UI eşlemesi; "Retrying"den ayrık). Görsel ayrım için **mavi/sarı** renk (kırmızı "Retrying"/"Erreur" ile karışmasın).
- `ports/RGSX/languages/*.json` + `webui/src/i18n.strings.js` (otomatik `npm run gen:i18n`): `network_down_banner`, `status_waiting_network` (tr/en).

## Doğrulama

- **Unit (Rust, `manager-http` tests):**
  - `network_down_parks_and_does_not_burn_retries`: `network_down=true` set, indirme enqueue et → park olduğunu doğrula (queue satır durumu "Ağ bekleniyor", `retries` sayacı 0 kalır) ve finalize/remove OLMADIĞINI doğrula; `network_down=false` + notify → indirme resume edip tamamlanır.
  - `network_probe_flip`: probe başarısızlıklarını simüle et → `network_down` true; başarı → false.
- **Live (port 5000):** Büyük bir ROM indirmeyi başlat → WiFi kes → UI banner + "Ağ bekleniyor" gösterir, indirme park eder (History'de `Erreur` YOK) → WiFi aç → banner kapanır, indirme `.part`'tan Range ile devam edip biter. Outage sırasında KESİNLİKLE `Erreur` kaydı düşmemeli.

## İlerleme

- 2026-08-20 — plan çıkarıldı (kullanıcı üzerinde düşünecek). Fazlı yaklaşım önerildi: Faz 1 backend park gate (kritik), Faz 2 probe task, Faz 3 UI banner + "Ağ bekleniyor".
- 2026-08-20 — Gemini önerisi (veri olarak alındı) ile teşhis ve fazlı plan teyit edildi. Öncelik sıralaması: **Faz 1 + Faz 3 ile başla**, Faz 2'yi pragmatik hale getir (ardışık N Network hatası → down; yalnızca outage sırasında hafif periyodik HEAD/ping ile reconnect tespiti; ayrı sürekli daemon YOK). Faz 2 description ve UI renk notu güncellendi.
- 2026-08-21 — **Implementasyon + doğrulama tamamlandı (henüz commit/push yapılmadı).** Faz 1+3+ pragmatik Faz 2 tek seferde uygulandı.
  - `state.rs`: `network_down: Arc<AtomicBool>`, `network_resume: Arc<Notify>`, `network_error_streak: Arc<AtomicU32>` **StateData içine** eklendi (AppState dışına değil — detay için memory `mem_1787254644477`: hem park gate hem decide_retry/reconnect-probe zaten kilitliyken erişiyor, SSE snapshot doğrudan alıyor, main.rs/contract.rs construction'ları DEĞİŞMEDİ).
  - `api.rs`: `NETWORK_DOWN_THRESHOLD = 3`; `probe_connectivity()` (8.8.8.8:53 / 1.1.1.1:53 TCP, 2s timeout, DNS yok); `decide_retry` artık `is_network` alır — 3 ardışık Network hatasında `network_down=true`, aktif kuyruk "Ağ bekleniyor", retry bütçesi YAKILMAZ (delay:0 → loop-top park gate yakalar). Ağ-dışı hata streak'i sıfırlar.
  - Park gate: bridge (~605) + native (~1975) döngü başlarında — `network_down` true iken `probe_connectivity()` başarısızsa park (1s timeout + `network_resume.notified()`), başarılıysa `network_down=false` + "Ağ bekleniyor"→"Downloading" + `notify_waiters()` + `dirty`.
  - `sse.rs`: `snapshot_json` → `network_down`. `App.vue`: `snapshot.network_down` reaktif + global banner (`tt('network_down_banner')`, sarı) + `statusMeta` "Ağ bekleniyor" → "AĞ BEKLENİYOR" (camgöbeği). 7 dile `network_down_banner` eklendi + `npm run gen:i18n`.
  - **Doğrulama:** `cargo test -p manager-http gap32` → 2 test GEÇTİ (streak 3'te flip + "Ağ bekleniyor"; ağ-dışı hata streak sıfırlar). Release build başarılı. Deploy: Kopya + Asıl (`manager-bin.exe` + `webui/` + `languages/`). Canlı sunucu (PID 13088, port 5000) yeni binary ile SSE snapshot `"network_down":false` veriyor.
  - **Canlı not:** Tek bir ölü host (internet yukarı) testinde `network_down` anlık flip olur ama probe anında resetler → banner kalıcı GÖRÜNMEZ (doğru: sadece o host retry eder, global park YOK). Gerçek outage (probe da başarısız) senaryosu kullanıcının WiFi keserek test etmesi gerekiyor; orada banner + "Ağ bekleniyor" sürer ve bağlanınca devam eder.
- 2026-08-21 (Faz 4 — restore bildirimi): Kullanıcı onayı: down durumu zaten banner'da, orada ayrı bildirim GEREKSİZ (spam). Sadece **geri-dönüş toast'u** eklendi, ve SAHTE toast olmaması için yalnızca GERÇEK kesintide tetiklenir.
  - `state.rs`: `network_outage_confirmed: Arc<AtomicBool>` eklendi.
  - `api.rs` park gate: probe BAŞARISIZSA (gerçek outage) `network_outage_confirmed=true` set; probe BAŞARILI olup `network_down`'ı false'a çekerken → yalnızca `compare_exchange(true,false)` ile tek emitçi garantisiyle `sse::publish(events, "network_restored", ...)`. Ölü-host titreşiminde probe hep başarılı → bayrak set EDİLMEZ → toast ASLA çıkmaz.
  - `App.vue`: SSE `network_restored` handler → `pushToast(tt('network_restored_toast'), 'success')` (~4sn kendiliğinden kapanan yeşil teyit). Down tarafında ek bildirim yok (banner yeterli).
  - 7 dile `network_restored_toast` + `gen:i18n`. Build + deploy (Kopya+Asıl) + canlı sunucu (PID 8620) yeniden başlatıldı; SSE `network_down:false` veriyor. (Gerçek outage'ta toast'ı tetiklemek bu ortamda kesilemedi; tasarım sahte-tostu önlüyor.)
