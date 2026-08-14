# TASK-002-GAP-1 — Nihai Teknik Spesifikasyon (KEEP_CONTRACT) — DOĞRULANMIŞ

**Dal:** custom • **Tarih:** 2026-08-14 (yeniden doğrulama) •
**Karar:** `retry_count`/`max_retries` `/api/download` **REQUEST** contract'ına eklenmez.

> Bu belge `custom` dalındaki GERÇEK kodun taze okunmasıyla (api.rs / state.rs / http/mod.rs /
> queue.py / download_state.py / test_api_contract.py) doğrulanmış, uygulanabilir nihai
> spesifikasyondur. Önceki `TASK-002-gap-1-nihai-teknik-spesifikasyon.md` (commit ed55347/4576494)
> ile karşılaştırılmış; **hatalı/eksik 3 nokta düzeltilmiş, 3 tasarım kararı "BELİRSİZ" olarak
> işaretlenmiştir** (kodlamaya başlamadan önce çözülmeli).

---

## 1. DOWNLOAD AKIŞI (gerçek kod — doğrulandı)

```
POST /api/download
  └─ handler: manager-http/src/api.rs:404  download(State, Json<Value>)
       ├─ request parse (UNTYPED Value):
       │    body.get("url")            api.rs:409
       │    body.get("platform")       api.rs:452
       │    body.get("game_index")     api.rs:453
       │    body.get("game_name")      api.rs:454
       │    body.get("dest_path")      api.rs:485
       │    → retry_count / max_retries YOK (KEEP_CONTRACT)
       │
       ├─ intercept_locally = is_torrent_url(url) && bridge var  api.rs:410
       ├─ RGSX_NATIVE_DOWNLOAD=1 && !torrent → native_ddl_download  api.rs:437
       └─ !intercept && catalog var     → Python proxy (history Python'da)  api.rs:446-450

TORRENT YOLU (bridge varsa):
  api.rs:494  tokio::spawn {
      bridge.download_torrent_progress(&u,&dest_path,Some(t),on_progress).await  api.rs:528
        Ok  → finalize_download_in_state(&state2,&t,&u,&n,&p,true,...)   api.rs:533
        Err → finalize_download_in_state(&state2,&t,&u,&n,&p,false,...)  api.rs:540
  }

DDL YOLU (native):
  api.rs:1101  native_ddl_download(...)
    api.rs:1152  tokio::spawn {
        req = DownloadRequest{...}                                  api.rs:1157-1163
        HttpDownloader::new()
            .with_progress(...)
            .download_async(&req).await                              api.rs:1164-1181
          Ok  → finalize_download_in_state(...true...)   api.rs:1185
          Err → finalize_download_in_state(...false, &e.message())  api.rs:1197

ORTAK SONUÇLANDIRMA:
  api.rs:1044-1096  finalize_download_in_state(state,task_id,url,name,plat,ok,msg)
      - history entry (task_id ile bulur: api.rs:1058) → yazar: status, message, progress  (1060-1064)
      - queue'dan çıkarır (1066-1071), downloaded listesine ekler (1073-1082), progress yazar (1083-1089)
      - SSE yayar (1090-1095)
      - entity_state / retry_count / max_retries / retry_at YAZMAZ  ⚠ parity açığı

HISTORY RESPONSE:
  api.rs:221-237  history handler → data.history klonlar + message noise strip → döner
      (catalog var → Python proxy; o zaman retry Python'da zaten var)
```

**Entegrasyon noktası:** Her iki dal da `finalize_download_in_state(ok, msg)` ile biter. Retry
motoru, `bridge.download_torrent_progress` (api.rs:528) ve `HttpDownloader::download_async`
(api.rs:1164) çağrılarını **saran retry döngüsü** olmalı; her denemede history güncellenir (Python parity).

---

## 2. RETRY MİMARİSİ (Python vs Rust — doğrulandı)

### Python (referans — `queue.py:440-628`, `download_state.py:147-326`)
| Kavram | Kaynak | Rust'ta? |
|---|---|---|
| `retry_count` | entry'den okunur/yazılır (`queue.py:482,534`) | ❌ yok |
| `max_retries` | `config.DOWNLOAD_MAX_RETRIES` default 3 (`config.py:176`, `queue.py:444-445,484`) | ❌ yok (sabit tanımlanacak) |
| `retry_at` | `now+delay` (`queue.py:517,541`) | ❌ yok |
| `entity_state` | `RETRY_SCHEDULED` (`queue.py:538`) | ❌ `finalize` yazmıyor |
| `classify_error` | `download_state.py:195` | ❌ yok |
| backoff | `retry_backoff_seconds = min(base*2^(n-1), max)` (`download_state.py:250`, base=5.0 max=300.0) | ❌ yok |
| scheduling | `_schedule_download_retry` thread → sleep(delay, 0.5 döngü) → slot bekle → **yeni task_id** → `download_rom` (`queue.py:572-628`) | ❌ yok |
| dedup/in-flight | `_retry_in_flight` set, **key=url** (`queue.py:441,603-605`) | ❌ yok |
| cancel/shutdown | `_app_shutting_down` + `cancel_events[task_id]` (`queue.py:584-596`) | ❌ (bkz. BELİRSİZ-2) |
| exhaustion | `retry_count>=max_retries` ∨ permanent → `FAILED_PERMANENT` (`queue.py:514,553-569`) | ❌ yok |

### Rust mevcut
- **State machine HAZIR** (`manager-core/src/state.rs`): varyantlar `RetryScheduled`(45),
  `FailedTransient`(52), `FailedPermanent`(53); event'ler `TransientFailure`(66),
  `RetryTriggered`(68), `RetryExhausted`(69); transition `transition()` (264-305) **21 kural**
  (test `transition_valid_table_matches_python` 433. satırda 21'i doğrular). **Motor yok** — tetiklenmiyor.
- **`manager-core/src/retry.rs` YOK** (grep + ls doğrulandı).
- **`manager-torrent/src/lib.rs` retry YOK.**
- **⚠️ TRANSPORT-LEVEL RETRY MEVCUT:** `manager-download/src/http/mod.rs` `HttpDownloader` iç retry
  döngüsü: `with_retry` (173), **default `max_retries=5`** (143), `base_backoff=5s` (144);
  `download_async` içinde bağlantı hatası (298-305)/429 (312-328)/5xx (330-337) yeniden dener.
  **Önemli:** api.rs:1164 şu an `with_retry` ÇAĞIRMIYOR → default 5 kullanılıyor. Yani reconcile
  (`with_retry(1)`) HENÜZ UYGULANMADI, planlı değişiklik (bkz. §6).
  - 403 dalı (338-356) header-variant/alt-URL fallback yapar ve `max_retries`'a BAĞLI DEĞİL;
    `with_retry(1)` sonrası da korunur.

---

## 3. API CONTRACT KARARI (KEEP_CONTRACT — kanıtlandı)

**SORU:** `retry_count`/`max_retries` `/api/download` REQUEST contract'ına eklenmeli mi?
**CEVAP: HAYIR.** Kanıt:
- Python worker `rgsx_manager.py:347-355` yalnızca `platform/game_index/game_name/url/mode` okur — retry yok.
- Rust handler `api.rs:404` untyped `Json<Value>`, yalnızca `url/game_name/platform/game_index/dest_path` okur — retry yok.
- Contract testleri **retry alanı içermez**: `test_api_contract.py:311/318/326` (download parametre testleri); dosyada `retry` grep'i boş. Rust `contract.rs:283/294/301` aynı şekilde.

**Nereye ait:**
- `max_retries` → Rust **constant** `DEFAULT_MAX_RETRIES = 3` (yeni `manager-core/src/retry.rs`; parity: `config.py:176`). Request'ten/ env'den gelmez (gap-1 kapsamı dışı).
- `retry_count` → Rust **internal state**, URL başına: `StateData.retries: HashMap<String,u32>` (§4) + history entry aynası.
- `retry_at` → Rust internal, `now + backoff` hesaplanır, history entry'ye yazılır.
- `/api/history` response → **taşımalı:** `entity_state`, `retry_count`, `max_retries`, `retry_at`.
  Python `apply_to_history_entry` (download_state.py:**312-326**, eski spec 230-248 demişti — YANLIŞ,
  gerçek 312-326) yazar. Rust finalize şu an yazmıyor → parity açığı. Bunlar **additive**; mevcut
  contract testleri (`test_api_contract.py:199-232` history testleri full-shape assert ETMEZ;
  `contract.rs:866/886` yalnız `status`/`progress` kontrol eder) kırılmaz.

**Düzeltme:** Eski spec "105 contract testi" demişti. Gerçek: `test_api_contract.py`=**54** test,
tüm suite (test_api_contract + test_download_batch + test_rgsx_manager) = **163** test. Sayı düzeltildi;
iddia (request değişmez) değişmedi.

---

## 4. RETRY IDENTITY — `HashMap<String,u32>` + `HashSet<String>` nerede?

**Karar: `manager-http/src/state.rs` içindeki `StateData`'ya eklenir** (AppState'a ayrı Arc<Mutex> değil).
- `StateData` (state.rs:22-41) zaten indirme kapsamlı mutable state tutuyor (history/queue/progress/downloaded).
- `AppState` (state.rs:87-98) `Arc<RwLock<StateData>>` sarmalı; alan eklemek ikinci kilide gerek bırakmaz.
- **Key = `game_url`** — Python `_retry_in_flight` key=url ile aynı (`queue.py:603`). Bir URL = bir job.

**Erişim / kritik bölüm:** Mevcut stil (api.rs history kısa write → guard drop → await). Retry döngüsü:
1. lock → `retry_in_flight` içermiyor mu + insert, `retries[url]+=1` → **drop** → `sleep(delay)` (lock YOK) → denemeyi tekrarla.
2. deneme bitince (başarı/permanent/exhausted): lock → `retry_in_flight.remove(url)` → drop.

### BELİRSİZ-1 — `task_id` yeniden üretilsin mi? (kullanıcının §4 sorusu, eski spec CEVAPSIZ)
- **Python:** her retry'de **YENİ `task_id` üretir** (`queue.py:610` `new_task_id = f"retry_{ts}_{hash}"`) ve
  `download_rom(url, ..., new_task_id)` ile yeni indirme akışına girer → **deneme başına ayrı history entry**.
- **Eski spec'in Rust taslağı:** tek `task_id` + tek history entry, `retry_count` yerinde artar.
- **Etki:** `/api/history` şekli değişir — Python N entry (her deneme ayrı), taslak 1 entry (`retry_count` alanıyla).
  **Bu, parity'nin görünür davranışını değiştirir.** Kodlamaya başlamadan KULLANICI karar vermeli:
  (a) Rust da her denemede yeni `task_id` + yeni history entry üretsin (Python ile birebir history şekli) mı,
  (b) yoksa tek entry + `retry_count` alanı (daha sade, ama Python'dan farklı history görünümü) mı?
  **BELİRSİZ — şu sebeple: history entry eşlemesi ve /api/history contract şekli buna bağlı.**

### BELİRSİZ-2 — Cancel/shutdown retry sleep'ini nasıl kesecek? (kullanıcının §4 sorusu, eski spec CEVAPSIZ)
- **Python:** `_app_shutting_down` global bayrak + `cancel_events[task_id]` (`queue.py:584-596`) runner thread'i döngü içinde kontrol eder.
- **Rust mevcut:** `AppState`/`StateData`'da **global shutdown sinyali YOK**. `/api/cancel` → torrent için
  `bridge.cancel_torrent(task_id)` (api.rs:619); **DDL için cancel hiç bağlı değil** (HttpDownloader `CancelFlag`
  var: `manager-download/src/http/stream.rs:42` `new/set/is_set`, `mod.rs:161 with_cancel`, `mod.rs:284`
  kontrol eder — ama `native_ddl_download` bunu `with_cancel` ile kurmuyor).
- **Karar gerekiyor:** Retry backoff `sleep`'i kesmek için `tokio::select!` önerilir:
  - DDL: `native_ddl_download` bir `CancelFlag` oluşturup `with_cancel`'a verir; retry döngüsü
    `tokio::select! { sleep(delay) , cancel_flag.set() => break }`.
  - Torrent: `/api/cancel` ile gelen `task_id` (veya url) bir `HashSet`/`CancelFlag` eşlemesine düşürülür;
    retry döngüsü o sinyali izler.
  - Global shutdown: `AppState`'e `Arc<Notify>` (veya `broadcast::Sender`) eklenip `/api/shutdown`
    (api.rs:815) bunu tetikler; retry `select!`'e katılır.
  **BELİRSİZ — şu sebeple: mevcut kodda global shutdown sinyali ve DDL-cancel bağlantısı yok; mekanizma
  seçilip bağlanmalı (tokio::select! + CancelFlag/Notify).**

### State machine bağlantısı (değişmez — yeniden icat edilmez)
- transient fail → `Downloading+TransientFailure→FailedTransient`, `FailedTransient+RetryTriggered→RetryScheduled`.
- retry başlarken → `RetryScheduled+Started→Downloading`.
- permanent/exhausted → `FailedTransient/RetryScheduled+PermanentFailure (veya RetryExhausted)→FailedPermanent`.
- History yazımı: `entity_state="RETRY_SCHEDULED"`→status `"Téléchargement"` (state.rs:214),
  `FAILED_PERMANENT`→status `"Erreur"` (state.rs:216).

---

## 5. CLASSIFY_ERROR PARITY (literal içerik — doğrulandı)

`download_state.py` marker listeleri (satır numarası DEĞİL, içerik):

```
_TRANSIENT_HTTP_STATUS = frozenset({408,409,425,429,500,502,503,504,520,521,522,523,524,525,526,527})

_PERMANENT_HTTP_STATUS = frozenset({400,401,402,403,404,405,406,410,411,412,413,414,415,416,417,418,
                                     422,423,424,426,428,431,451})

_PERMANENT_MARKERS = (
    "access denied","accès refusé","access refused",
    "authentication required","auth required","unauthorized","forbidden",
    "browser challenge","interactive browser session",
    "payload is not a valid archive","not a valid archive","valid archive signature",
    "html/challenge content","downloaded html",
    "empty response",
    "restricted (is_dark","is_dark=true",
    "file not found","introuvable","not found","has been removed",
    "removed for abuse","piracy domain",
    "password incorrect","invalid password","mot de passe",
    "pas assez d'espace","insufficient disk space","low disk space",
    "manque d'espace",
)

_TRANSIENT_MARKERS = (
    "timeout","timed out","timed-out","read timed",
    "connection error","connexion","connection aborted","connection reset",
    "connection refused","connection timed","unable to connect","cannot connect",
    "max retries exceeded","retries exceeded",
    "rate limit","too many requests","temporarily unavailable",
    "server error","erreur serveur","service unavailable","bad gateway",
    "gateway time-out","limits downloads to one","limite les téléchargements",
    "link appears down","temporary failure","ressayer","réessayez",
    "essayez plus tard","slow down","n'existait pas","temporairement",
)
```

`classify_error(message, error_type=None)` mantığı (download_state.py:195-229):
1. `error_type` verilirse: `InsufficientDiskSpace` → permanent(False); tip adında "Timeout"/"Connection" → transient(True).
2. `text = message.lower()`; boşsa → permanent(False).
3. **PERMANENT_MARKERS önce** (her zaman) → False.
4. Çıkarılan HTTP kodları: transient set'te → True; permanent set'te → False.
5. TRANSIENT_MARKERS → True.
6. Varsayılan → **False (permanent)** — sonsuz döngü önlemek için.

`retry_backoff_seconds(retry_count, base=5.0, max_wait=300.0)` → `0.0` if `retry_count<=0` else `min(base*2^(retry_count-1), max_wait)`.

### ÇAKIŞMA-3 — `classify_error` dili UYUŞMUYOR (kodlamadan önce çözülmesi zorunlu)
- `HttpDownloader` **Türkçe** hata string'leri üretir (`mod.rs:44-63`):
  `Network("bağlantı: ...")`, `BrowserChallenge("browser challenge tespit edildi...")`,
  `HtmlInsteadOfPayload("HTML/challenge içerik arşiv yerine indirildi: {0}")`,
  `Http("HTTP 429 (rate-limit, N hits)")`, `Http("HTTP 500")`.
- Yukarıdaki Python marker'ları **İngilizce + Fransızca**; Rust string'leri Türkçe.
- "Birebir port" (sadece İngilizce/Fransızca marker taraması) Rust hatalarının çoğunu YANLIŞ
  sınıflandırır (ör. `"bağlantı"` ne transient ne permanent marker'a düşer → varsayılan **permanent** →
  retry hiç tetiklenmez; bu KRİTİK bir regresyon olur).
- **Gerekli çözüm:** Rust `classify_error`'u **enum varyantı + durum kodu çıkarımı** üzerinden yaz:
  - `DownloadError::Network` → transient; `::BrowserChallenge`/`::InvalidArchive`/`::PartialArchiveRejected`/
    `::HtmlInsteadOfPayload`/`::EmptyResponse`/`::InsufficientDiskSpace` → permanent; `::Http(msg)` →
    msg içinden HTTP kodu çıkar (429/5xx→transient, 401/403/404→permanent).
  - `BridgeError` (torrent) zaten enum: `Timeout`→transient, `Rpc/Protocol/Spwn/Io`→ mesaja göre.
  - Marker listeleri **Türkçe karşılıklarla** da zenginleştirilir (parity için İngilizce/Fransızca korunur +
    Türkçe eklenir). Bu, "birebir string port" iddiasının revize edilmesi demektir.

---

## 6. TRANSPORT-RETRY RECONCILE (doğrulandı)

- `HttpDownloader` iç retry default `max_retries=5` (mod.rs:143); api.rs:1164 şu an `with_retry`
  çağırmadığı için **5 deneme** yapıyor.
- Python job-level: `1 + max_retries(3) = 4` toplam deneme.
- **Karar:** DDL yolunda `HttpDownloader::new().with_retry(1, Duration::from_secs(5))` (api.rs:1164)
  → inner tek deneme; tüm bütçe job-level `max_retries=3`'te toplanır → **1×4 = 4** (Python ile eş).
- 403 header-variant/alt-URL fallback (mod.rs:338-356) `max_retries`'a bağlı DEĞİL → `with_retry(1)`
  sonrası da korunur (provider negotiation kaybı olmaz).
- Torrent yolunda inner retry zaten yok; yalnız job-level uygulanır.
- **Not:** `with_retry(1)` ile bile 403 dalı birden çok variant deneyebilir; bu "retry" değil
  provider-specific negotiation sayılır, toplam deneme sayısı Python'la yakınsar (4±variant).

---

## 7. ÖZET KARAR (numaralı liste)

### Değişecek dosyalar
1. `manager-core/src/retry.rs` (**YENİ**) — `enum ErrorClass`, `classify_error(DownloadError|BridgeError)`,
   `retry_backoff_seconds`, sabitler `DEFAULT_MAX_RETRIES=3`, `DEFAULT_BACKOFF_BASE_SEC=5.0`,
   `DEFAULT_BACKOFF_MAX_SEC=300.0`. **ÇAKIŞMA-3 gereği:** string-marker port DEĞİL, enum+status
   sınıflandırması + Türkçe marker zenginleştirme ile yazılacak.
2. `manager-core/src/lib.rs` — `pub mod retry;`
3. `manager-http/src/state.rs` — `StateData`'ya `retries: HashMap<String,u32>` + `retry_in_flight: HashSet<String>` (key=game_url).
4. `manager-http/src/api.rs` —
   - torrent spawn (api.rs:494-544) ve DDL spawn (api.rs:1152-1209) **ortak retry döngüsü** ile sarılır;
   - `finalize_download_in_state` (api.rs:1044) → retry alanlarını yazar
     (`entity_state`/`retry_count`/`max_retries`/`retry_at`/`error`);
   - DDL yolunda `HttpDownloader::with_retry(1, Duration::from_secs(5))` (§6).
5. `manager-http/src/api.rs` (cancel/shutdown) — BELİRSİZ-2 çözümü: `tokio::select!` + `CancelFlag`/`Notify`.

### Eklenecek alanlar
- `StateData.retries`, `StateData.retry_in_flight` (state.rs).
- history entry: `entity_state`, `retry_count`, `max_retries`, `retry_at` (finalize ile yazılır; additive).
- `(opsiyonel, BELİRSİZ-1'e bağlı)` her denemede yeni `task_id` + yeni history entry.

### Yazılacak testler
- `manager-core` retry birim testleri: `classify_error` (Network→transient, BrowserChallenge→permanent,
  Http 429/5xx→transient, Http 403/404→permanent, Türkçe mesaj parity), `retry_backoff_seconds`
  (`min(base*2^(n-1),max)`, `n<=0→0`).
- state machine zaten 21 transition testiyle kapsanıyor (state.rs testleri) — yeni test gerekmez.
- `manager-http` contract testleri **değişmez** (request contract dokunulmaz; history additive).
  Mevcut `contract.rs` history testleri full-shape assert ETMEDİĞİ için güvenli.

### BELİRSİZ / çözülmesi zorunlu maddeler
- **BELİRSİZ-1:** `task_id` her retry'de yeniden mi üretilsin (Python parity, N history entry) yoksa
  tek entry + `retry_count` mı? — Kullanıcı kararı gerekli.
- **BELİRSİZ-2:** Retry sleep cancel/shutdown kesme mekanizması (global `Notify` + `CancelFlag` +
  `/api/cancel` bağlantısı) — mevcut kodda yok, seçilip bağlanmalı.
- **ÇAKIŞMA-3:** `classify_error` "birebir string port" olamaz; Rust Türkçe hataları yüzünden enum+
  status sınıflandırmasına çevrilmeli — spec'in "birebir port" ifadesi revize edildi.

### Doğrulanan (değişmeyen) kararlar
- REQUEST contract değişmez (kanıt: rgsx_manager.py:347-355, api.rs:404, contract testleri).
- State machine yeniden icat edilmez; motor eksik olanı üretir.
- `key=game_url`; `StateData` içinde tutulur; lock sleep sırasında tutulmaz.
- Transport-retry reconcile: `with_retry(1)` + job-level `max_retries=3` → toplam 4 deneme.
