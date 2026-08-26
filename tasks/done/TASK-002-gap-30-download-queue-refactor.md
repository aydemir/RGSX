# TASK-002-gap-30 — Download Queue Architecture Refactoring & Performance

- **id:** TASK-002-gap-30
- **title:** Download Queue refactoring — persistent worker, gated dispatch, O(1) bulk ingest
- **status:** done
- **priority:** P1
- **created:** 2026-08-19
- **environment:** both
- **tags:** manager-http, queue, state-machine, performance, sse

## Kaynak

- Kullanıcı tarafından doğrudan verilen teknik spec (aşağıda "Acceptance Criteria" ile).
  Mevcut kod tabanı: `manager-rs/manager-http/src/api.rs`, `manager-rs/manager-http/src/state.rs`,
  `webui/src/App.vue`. Önceki adım `commit 9d49433` (non-incremental batch enqueue via pending-set
  + consumer) ile kıyaslandı; spec ile 6 noktada sapma/eksik bulundu.

## Açıklama

Spec, 1000+ öğenin UI/scan döngüsünü bloklamadan kuyruğa alınmasını ve Paused/Stopped durumunun
gelen görevleri düşürmemesini / worker event loop'unu sonlandırmamasını hedefler. Mevcut
uygulama (`9d49433`) iyi bir ilk adımdır (handler bloke etmez, consumer sürekli döner, stop-all
consumer'ı durdurur) ama spec'in öngördüğü mimariye tam uymuyor:

1. **Mimarî sapma:** `mpsc::Receiver<QueueCommand>` + state machine yok; kod `VecDeque` + `Notify`
   (polling) kullanıyor. `QueueCommand::{Add, AddBatch}` varyantı yok.
2. **KRİTİK HATA — Paused'da dispatch gate'lenmiyor:** `download_consumer` (`api.rs:853`) pop edip
   doğrudan `download()` çağırıyor; `global_paused` kontrolü yok. Native `download()` (`api.rs:580`
   spawn) da `global_paused`'a bakmıyor → Paused'da "Download All" indirmeyi anında başlatıyor,
   pause bypass oluyor. Spec: Paused'da Add kabul edilip buffer'a yazılmalı, pop/dispatch `!Paused`
   ile gate'lenmeli; resume'da baştan işlenmeli.
3. **Lock contention:** `download_batch` (`api.rs:813-816`) her öğe için ayrı `state.write()` alıp
   `push_back` yapıyor. Spec: lock bir kez alınmalı, `tasks.extend(batch)` ile bırakılmalı.
4. **O(N) veri yapıları:** `queue: Vec<Value>` (`state.rs:30`) → üyelik taraması O(N). `HashMap<TaskId,
   TaskState>` / `BTreeMap` yok. `downloaded` kontrolü (`api.rs:770-776`) her batch'te array tarayıp
   HashSet'e çeviriyor → O(N).
5. **UI non-blocking read eksik:** `games`/`queue` endpoint'leri worker lock'u (`state.read()`) ile
   contended. Spec: atomic `HashSet<TaskId>` snapshot istiyor.
6. **Queue SSE tick yok:** progress 250ms throttle'lı (`api.rs:643`) ama queue-size/active metrikleri
   yalnız olay anında yayınlanıyor; sabit tick'te batched delta değil.

## Kapsam / Dosyalar

- `manager-rs/manager-http/src/state.rs` — `QueueCommand`, `QueueStatus`, `TaskState` enum'ları;
  `queue: VecDeque<TaskId>` + `tasks: HashMap<TaskId, TaskState>`; `downloaded` indeks
  (`HashSet<(platform,name)>`); `queued_ids: Arc<RwLock<HashSet<TaskId>>>` snapshot;
  `tx: mpsc::Sender<QueueCommand>` + `resume_notify: Arc<Notify>`; `consumer_started` kaldırılır.
- `manager-rs/manager-http/src/api.rs` — `download_batch` (tek `extend`), `download_consumer`
  (gated dispatch + Paused buffer + resume notify), `pause`/`resume` (state machine + notify_waiters),
  `queue_clear` (stop-all), `games`/`queue` endpoint (snapshot read), queue SSE tick.
- `manager-rs/manager-bin/src/main.rs` — worker spawn bağlantısı (gerekirse).
- `webui/src/App.vue` — mevcut `downloadAll` → `/api/download/batch` çağrısı korunur (değişiklik
  minimal; yalnızca queue snapshot tüketimi varsa).
- `manager-rs/manager-http/tests/contract.rs` — 114 contract + 21 lib korunur; yeni testler:
  paused-buffer-resume, bulk-extend O(1), stop-all idempotent.

## Doğrulama

- `cargo check` ve `cargo test` (manager-http) — sıfır warning, 21 lib + 114 contract PASS.
- `cargo check --target x86_64-pc-windows-gnu` (cross) — Windows derlemesi kırılmamalı.
- Canlı senaryo: büyük liste (1000+) "Download All" → anında dönüş, liste navigasyonu donmaz.
- Paused iken "Download All" → görevler buffer'a yazılır, resume'da baştan işlenir (pause bypass yok).
- stop-all → taze büyüme durur, idempotent.
- Liste render worker lock'una girmeden O(1) "queued?" bakar.

## Uygulama Planı (faz sıralı)

- **F1 — Command channel + state machine** (`state.rs` + `api.rs`): `enum QueueCommand { Add, AddBatch }`
  + `enum QueueStatus { Running, Paused, Stopped }`; `AppState`'e `tx` + worker `rx` (tokio::spawn).
- **F2 — Gated dispatch + Paused buffer** (`download_consumer`): `loop { select!{ cmd=rx.recv(),
  notify=resume_notify.notified() } }`; dispatch `if status==Paused { buffer.push_back(item); continue; }`;
  `Paused→Running` geçişinde `Notify::notify_waiters()`.
- **F3 — Tek `extend` bulk insert** (`api.rs:789-819`): `download_batch` lock'u bir kez alır,
  `VecDeque::extend(batch)` yapar, bırakır.
- **F4 — O(1) veri yapıları** (`state.rs`): `queue: VecDeque<TaskId>` + `tasks: HashMap<TaskId,
  TaskState>`; `downloaded` için `HashSet<(platform,name)>` indeks.
- **F5 — Atomic snapshot** (`state.rs` + `games`/`queue`): `queued_ids: Arc<RwLock<HashSet<TaskId>>>`
  read-only snapshot; liste render worker lock'una girmeden O(1) bakar.
- **F6 — Queue SSE tick** (`api.rs` + `sse`): queue-size/active metrikleri 250–500ms
  `tokio::time::interval` ile batched delta yayını.
- **F7 — Contract + lib testleri**: korunur + yeni (paused-buffer-resume, bulk-extend O(1),
  stop-all-idempotent).

---

## İlerleme

- 2026-08-19 — Yeniden değerlendirme tamamlandı; 6 sapma tespit edildi, plan 7 faza bölündü, task'a
  alındı. Uygulama kullanıcı onayı bekliyor.
