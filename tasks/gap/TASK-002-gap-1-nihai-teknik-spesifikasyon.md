# TASK-002-GAP-1 — Nihai Teknik Spesifikasyon (KEEP_CONTRACT)

**Dal:** custom  •  **Tarih:** 2026-08-14  •  **Karar:** `retry_count`/`max_retries` `/api/download` REQUEST contract'ına eklenmez.

> Bu belge, repository kodunun (custom branch) ve mevcut test/contract'ların taze okunmasıyla
> çıkarılmış uygulanabilir teknik spesifikasyondur. Kod yazılmadan önce referans alınır.

---

## 1. DOWNLOAD AKIŞI (gerçek kod)

```
POST /api/download
  └─ handler: manager-http/src/api.rs:399  download(State(state), Json(body): Json<Value>)
       ├─ request parse (UNTYPED Value):
       │    body.get("url")            api.rs:407
       │    body.get("platform")       api.rs:456
       │    body.get("game_index")     api.rs:457
       │    body.get("game_name")      api.rs:458
       │    body.get("dest_path")      api.rs:469
       │    → retry_count / max_retries YOK
       │
       ├─ is_torrent_url && bridge     → yerel torrent dallı        api.rs:~407-409
       ├─ RGSX_NATIVE_DOWNLOAD=1 && !torrent → native_ddl_download api.rs:421-434
       └─ !intercept && state.catalog  → Python proxy (history Python'da) api.rs:447-452

TORRENT YOLU (bridge varsa):
  api.rs:462-545  tokio::spawn {
      bridge.download_torrent_progress(&u,&dest_path,Some(t),on_progress).await  api.rs:528
        Ok  → finalize_download_in_state(&state2,&t,&u,&n,&p,true,...)   api.rs:533
        Err → finalize_download_in_state(&state2,&t,&u,&n,&p,false,...)  api.rs:540
  }

DDL YOLU (native):
  api.rs:1101  native_ddl_download(...)
    api.rs:1152  tokio::spawn {
        HttpDownloader::new().with_progress(...).download_async(&req).await  api.rs:1164-1181
          Ok  → finalize_download_in_state(...true...)   api.rs:1185
          Err → finalize_download_in_state(...false...)  api.rs:1197

ORTAK SONUÇLANDIRMA:
  api.rs:1044-1096  finalize_download_in_state(state,task_id,url,name,plat,ok,msg)
      - history entry (task_id ile bul) → yazar: status, message, progress
      - queue'dan çıkarır, downloaded/progress günceller, SSE yayar
      - entity_state / retry_count / max_retries / retry_at YAZMAZ

HISTORY RESPONSE:
  api.rs:219-237  history handler → data.history (mesaj noise strip) döner
      (catalog varsa Python'a proxy; o zaman retry Python'da zaten var)
```

**Entegrasyon noktası:** Her iki dal da `finalize_download_in_state(ok: bool, msg)` ile bitiyor.
Retry motoru, `download_torrent_progress` (`:528`) ve `HttpDownloader::download_async` (`:1164`)
çağrısını **saran retry döngüsü** olmalı; her denemede history güncellenir (Python parity).

---

## 2. RETRY MİMARİSİ (Python vs Rust)

### Python (referans — `ports/RGSX/network/queue.py:468-629`, `download_state.py`)
| Kavram | Kaynak | Rust'ta? |
|---|---|---|
| `retry_count` | history entry'den okunur/yazılır (`queue.py:483,539`) | ❌ yok |
| `max_retries` | `config.DOWNLOAD_MAX_RETRIES` default 3 (`config.py:176`, `queue.py:444-445,484`) | ❌ yok (sabit tanımlanacak) |
| `retry_at` | `now+delay` (`queue.py:517,541`) | ❌ yok |
| `entity_state` | `RETRY_SCHEDULED` (`queue.py:538`) | ❌ `finalize` yazmıyor |
| `classify_error` | `download_state.py:195` (PERMANENT/TRANSIENT marker + HTTP kod) | ❌ yok |
| backoff | `retry_backoff_seconds = min(base*2^(n-1), max)` (`download_state.py:250`, base=5.0 max=300.0) | ❌ yok |
| scheduling | `_schedule_download_retry` thread → sleep(delay) → slot bekle → yeni task_id → `download_rom` (`queue.py:572-628`) | ❌ yok |
| dedup/in-flight | `_retry_in_flight` set, key=url (`queue.py:441,603-605`) | ❌ yok |
| cancel/shutdown | `_app_shutting_down` + `cancel_events[task_id]` kontrolü (`queue.py:584-596`) | ❌ yok (cancel mevcut, retry için bağlanacak) |
| exhaustion | `retry_count>=max_retries` ∨ permanent → `FAILED_PERMANENT` (`queue.py:553-569`) | ❌ yok |

### Rust mevcut
- **State machine HAZIR** (`manager-core/src/state.rs`): varyantlar `RetryScheduled`(`:51`),
  `FailedTransient`(`:52`), `FailedPermanent`; event'ler `TransientFailure`(`:66`),
  `RetryTriggered`(`:68`), `RetryExhausted`(`:69`); transition tablosu (`:289-301`):
  `(Downloading,TransientFailure)→FailedTransient`, `(FailedTransient,RetryTriggered)→RetryScheduled`,
  `(RetryScheduled,Started)→Downloading`, `(FailedTransient,PermanentFailure)→FailedPermanent`,
  `(FailedTransient,RetryExhausted)→FailedPermanent`, `(RetryScheduled,PermanentFailure)→FailedPermanent`.
  **Motor yok** — bunlar hiç tetiklenmiyor.
- **`manager-core/src/retry.rs` YOK** (src/ = contract, lib, settings, state, watchdog).
- **`manager-torrent/src/lib.rs` retry YOK** (grep temiz).
- **⚠️ ÇAKIŞMA RİSKİ — mevcut transport-level retry:** `manager-download/src/http/mod.rs`
  `HttpDownloader` zaten içinde retry döngüsü taşıyor: `with_retry`(`:173`), default
  `max_retries=5`(`:143`), içeride bağlantı hatası(`:~300`)/429(`:~319`)/5xx(`:~332`) için yeniden
  dener. Bu Python parity retry'si DEĞİL; gap-4'ün transport-dayanıklılık özelliği. Eğer gap-1
  job-level retry de eklenirse toplam deneme = 5(inner) × (1+3 job) = **20** vs Python'ın **4**
  (1+3). **Mutlaka reconcile edilmeli** (bkz. §4 karar).

---

## 3. API CONTRACT KARARI

**SORU:** `retry_count`/`max_retries` `/api/download` REQUEST contract'ına eklenmeli mi?
**CEVAP: HAYIR (KEEP_CONTRACT).** Kod kanıtı:
- Python worker `rgsx_manager.py:347-351` yalnızca `platform/game_index/game_name/url/mode` okur — retry yok.
- Rust handler `api.rs:399` untyped `Json<Value>`, yalnızca `url/game_name/platform/game_index/dest_path` okur — retry yok.
- Contract testi `test_api_contract.py:312,321,327` POST'ları yalnızca bu alanlarla; test dosyasında `retry` grep'i **boş**.

**Nereye ait:**
- `max_retries` → Rust **constant** `DEFAULT_MAX_RETRIES = 3` (yeni `manager-core` retry modülü; parity: `config.py:176`). Request'ten gelmez, env'e wiring gap-1 kapsamı dışı.
- `retry_count` → Rust **internal state**, URL başına, `StateData.retries: HashMap<String,u32>` (veya history entry). Request'ten gelmez.
- `retry_at` → Rust internal, `now + backoff` hesaplanır, history entry'ye yazılır. Request'ten gelmez.
- `/api/history` response → **taşımalı:** `entity_state`, `retry_count`, `max_retries`, `retry_at`. Python bunları yazar (`download_state.py:230-248 apply_to_history_entry`, `queue.py:537-543`). Rust şu an yazmıyor → parity açığı. Bunlar response'a **additive** eklenir; client bilmezse yok sayar, 105 contract testi assert etmediği için **kırılmaz**.

**Net ayrım:** REQUEST contract = değişmez. HISTORY/RESPONSE contract = Python parity'siyle bu 4 alan taşınır (additive).

---

## 4. RETRY IDENTITY — `HashMap<String,u32>` + `HashSet<String>` nerede?

**Yanıt: `manager-http/src/state.rs` içindeki `StateData`'ya eklenir (AppState'a ayrı Arc<Mutex> değil).**

Gerekçe:
- `StateData` (state.rs:22-41) zaten indirme kapsamlı mutable state tutuyor: `history`, `queue`,
  `progress`, `manager_state`. Retry bookkeeping aynı kategoride → birlikte tutulur.
- `AppState` (state.rs:86-98) zaten `Arc<RwLock<StateData>>` sarmalı; alan eklemek ikinci bir
  kilide gerek bırakmaz, tüm download state tek yerde kalır.
- **Key = `game_url`** (kanonik indirme URL'si) — Python'ın `_retry_in_flight` key=url ile aynı
  (`queue.py:603`). Bir URL = bir job. (Aynı URL retry döngüsü sırasında tekrar POST edilirse
  in-flight set dedupliker — Python davranışı.)
- **Erişim:** mevcut `state.write()`/`state.read()` RwLock ile, **yalnızca kısa kritik bölümler**;
  `tokio::time::sleep` sırasında lock TUTULMAZ (mevcut kod stili: api.rs history'yi kısa write ile
  yazar, sonra guard drop, sonra await). Pattern:
  - retry planlanırken: lock → `retry_in_flight` içermiyor mu kontrol + insert, `retries[url]+=1` → drop → `sleep(delay)` → denemeyi tekrarla.
  - deneme bitince (başarı/permanent/exhausted): lock → `retry_in_flight.remove(url)` → drop.
- `retry_in_flight` amacı: aynı URL için eşzamanlı ikinci bir retry thread'ini engellemek (Python `queue.py:602-605`).
- `retries` amacı: `entry["retry_count"]` karşılığı; `max_retries` ile karşılaştırılır.

**Transport-retry reconcile KARARI:** Job-level retry (`manager-http` katmanı) Python parity'sinin
**tek otoritesi** olmalı. Mevcut `HttpDownloader` iç retry'su (`mod.rs:143` default 5) job-level
engine ile çarpışmasın diye: gap-1 entegrasyonunda DDL yolundaki `HttpDownloader` çağrısına
**`with_retry(1, ...)`** (inner retry kapalı) geçirilir; tüm retry bütçesi job-level
`max_retries=3`'te toplanır → Python ile aynı deneme sayısı (1 + 3). Torrent yolunda zaten inner
retry yok, sadece job-level uygulanır.

**State machine bağlantısı:** Motor mevcut transition'ları üretir (yeniden icat etmez):
- transient fail → `Downloading +TransientFailure→ FailedTransient`, `FailedTransient +RetryTriggered→ RetryScheduled`; history `entity_state="RETRY_SCHEDULED"`, `status="Téléchargement"` (legacy map state.rs:214).
- retry denemesi başlarken → `RetryScheduled +Started→ Downloading`.
- permanent/exhausted → `FailedTransient/RetryScheduled +PermanentFailure (veya RetryExhausted)→ FailedPermanent`; history `entity_state="FAILED_PERMANENT"`, `status="Erreur"`.

---

## ÖZET KARAR

1. **REQUEST contract değişmez** — `retry_count`/`max_retries` eklenmez (kanıt: `rgsx_manager.py:347-351`, `api.rs:399`, `test_api_contract.py`).
2. **Retry motoru** = `manager-http/src/api.rs` içindeki iki spawn/finalize akışını saran ortak döngü; `manager-torrent`'e bağımsız motor yok.
3. **`classify_error` + `retry_backoff_seconds` + sabitler** = yeni `manager-core/src/retry.rs` (Python `download_state.py` birebir port: `_PERMANENT_MARKERS` `:157`, `_TRANSIENT_MARKERS` `:172`, `_TRANSIENT_HTTP_STATUS` `:151`, `_PERMANENT_HTTP_STATUS` `:153`).
4. **`retries: HashMap<String,u32>` + `retry_in_flight: HashSet<String>`** = `StateData` (`manager-http/src/state.rs`), key=`game_url`, kısa kritik bölüm erişimi.
5. **History parity** = `finalize_download_in_state` (api.rs:1044) retry alanlarını yazar (`entity_state/retry_count/max_retries/retry_at`); `/api/download` contract testi değişmez.
6. **Transport-retry reconcile** = DDL yolunda `HttpDownloader::with_retry(1,...)`; job-level `max_retries=3` otorite.

**Implementasyonda değişecek dosyalar:** `manager-core/src/retry.rs` (yeni), `manager-core/src/lib.rs`
(`pub mod retry`), `manager-http/src/state.rs` (`StateData` alanları), `manager-http/src/api.rs`
(retry döngüsü + finalize parity + `with_retry(1)`). Testler: `manager-core` retry birim testleri;
`test_api_contract.py` **değişmez**.
