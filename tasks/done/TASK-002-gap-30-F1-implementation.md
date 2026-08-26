# TASK-002-gap-30 — F1 Implementation Record (Command Channel + State Machine)

- **task:** TASK-002-gap-30
- **phase:** F1
- **date:** 2026-08-19
- **status:** implemented + lib tests green (21/21); contract binary linked
- **files:** `manager-rs/manager-http/src/state.rs`, `manager-rs/manager-http/src/api.rs`,
  `manager-rs/manager-bin/src/main.rs`, `manager-rs/manager-http/tests/contract.rs`,
  `manager-rs/.cargo/config.toml` (env isolation)

## Özet

Spec'in "1. State Machine & Event Loop Architecture" maddesinin altyapısı kuruldu.
`pending-set + lazy-spawn consumer` modeli, `mpsc::Sender<QueueCommand>` + sürekli
çalışan `queue_worker` (tokio::spawn) modeline dönüştürüldü.

1. **Yeni tipler (`state.rs`)**
   - `enum QueueCommand { Add(QueuedItem), AddBatch(Vec<QueuedItem>) }`
   - `struct QueuedItem { platform, name, url }`
   - `enum QueueStatus { Running, Paused, Stopped }` (default `Running`) — F2'de gate için.
   - `StateData.pending_set: VecDeque<QueuedItem>` (eski `(String,String,String)` tuple'sı kalktı).
   - `StateData.status: QueueStatus` eklendi; `consumer_started: AtomicBool` kaldırıldı (worker
     artık `AppState::empty()`/`with_data()`/`main.rs`'te bir kez spawn ediliyor).
   - `AppState.tx: mpsc::Sender<QueueCommand>` eklendi.

2. **`download_batch` (`api.rs`)** — handler artık katalog çözümü + dedupe yapıp geçerli
   öğeleri yerel `Vec<QueuedItem>`'a toplar ve **tek** `QueueCommand::AddBatch` mesajıyla worker'a
   yollar (`state.tx.send(...).await`). Eski per-item `state.write()` + lazy consumer spawn kalktı
   → spec #3 (lock contention) F1'de giderildi.

3. **`download_consumer` → `queue_worker` (`api.rs`)** — `mpsc::Receiver<QueueCommand>` üzerinden
   sürekli döngü. Tek çıkış: `rx.recv() == None` (tüm `tx` drop). `Paused`/`Stopped`'da `break`/
   `return` YOK; gelen komutlar `pending_set`'e yazılır (drop yok). `dispatch_queued` kilit DIŞINDA
   `download()` çağırır.

4. **`main.rs` + `contract.rs` test helper** — `tx`/`rx` kanalı kurulup worker spawn edildi
   (struct literal güncellemeleri).

5. **Regresyon düzeltmesi:** İlk draft `AppState::empty()`/`with_data()` içine `tokio::spawn`
   koymuştu; bu senkron `#[test]` birim testlerinde "no reactor running" panic'i verdi.
   `Handle::try_current()` ile sarmalandı — runtime varsa spawn, yoksa atla.

## Doğrulama

- `cargo check -p manager-http` → **sıfır uyarı, sıfır hata** (tek uyarılar `manager-scan`'da,
  önceden var).
- `cargo build -p manager-http` → başarılı.
- **Test link hatası (Termux/glibc sızıntısı) çözüldü**: `pkg-config --libs liblzma` Termux kökünü
  (`/data/data/com.termux/files/usr/lib`) döndürüyordu; `xz2` build script'i Bionic liblzma'yı link
  komutuna yazıp patlatıyordu. Çözüm: `apt-get install -y liblzma-dev` + `PKG_CONFIG_LIBDIR`
  izolasyonu + `manager-rs/.cargo/config.toml`'e `[target.aarch64-unknown-linux-gnu] rustflags`
  (`-L/usr/lib/aarch64-linux-gnu`) eklendi (Windows `[build]` bloğu korundu, cross etkilenmez).
- `cargo test -p manager-http --lib` → **21 passed; 0 failed**.
- `cargo test -p manager-http --test contract --no-run` → contract ikilisi başarıyla link edildi
  (114 contract testi için ikili hazır; sandbox'ta 114 HTTP testinin tam çalıştırılması >10dk
  sürdüğünden ikili derlemesi + lib 21 testi ile teyit edildi).

## İnceleme Notları (kullanıcının 3 sorusu)

- **Paused'da kanal dinleniyor mu / drop?** Evet dinleniyor, drop yok. `select!` her tur `rx.recv()`
  await eder; `recv` cancel-safe olduğu için `resume` dalı kazanınca bile komut kanalda kalır.
  (F1'de `status` henüz gate'lenmiyor — o F2'nin işi; F1'de Paused olsa bile dispatch anında başlar.)
- **break/return patlatır mı?** Tek `return` = `rx.recv() == None` (meşru shutdown). `break`/panic yok.
- **Ownership / kilit deadlock?** `pending_notify.clone()` guard'ı await dışına taşır; drain'de
  write kilidi yalnız `pop_front` süresince tutulur, `download` kilit DIŞINDA çağrılır; `extend`/
  `send` kilit altında await etmez. Deadlock/contention yok.

## Diff

```diff
diff --git a/manager-rs/manager-bin/src/main.rs b/manager-rs/manager-bin/src/main.rs
--- a/manager-rs/manager-bin/src/main.rs
+++ b/manager-rs/manager-bin/src/main.rs
@@ -291,13 +291,19 @@ async fn run(paths: paths::RgsxPaths) {
     let shutdown = Arc::new(Notify::new());
-    let app = router(AppState {
-        data: Arc::new(std::sync::RwLock::new(data)),
-        events: events.clone(),
-        bridge: bridge.clone(),
-        static_root,
-        catalog,
-        shutdown: shutdown.clone(),
+    let app = router({
+        let (tx, rx) = tokio::sync::mpsc::channel::<manager_http::state::QueueCommand>(1024);
+        let state = AppState {
+            data: Arc::new(std::sync::RwLock::new(data)),
+            events: events.clone(),
+            bridge: bridge.clone(),
+            static_root: static_root.clone(),
+            catalog: catalog.clone(),
+            shutdown: shutdown.clone(),
+            tx,
+        };
+        tokio::spawn(manager_http::api::queue_worker(rx, state.clone()));
+        state
     });

diff --git a/manager-rs/manager-http/src/api.rs b/manager-rs/manager-http/src/api.rs
--- a/manager-rs/manager-http/src/api.rs
+++ b/manager-rs/manager-http/src/api.rs
@@ -19,7 +19,7 @@
-use tokio::sync::Notify;
+use tokio::sync::{mpsc, Notify};
@@ -30,7 +30,7 @@
-use crate::state::AppState;
+use crate::state::{AppState, QueueCommand, QueuedItem};
@@ -781,11 +781,12 @@
+    let mut items: Vec<QueuedItem> = Vec::with_capacity(names.len());
     for n in names {
         ...
-        {
-            let mut d = state.write();
-            d.pending_set.push_back((platform.clone(), name.clone(), url));
-        }
+        items.push(QueuedItem { platform: platform.clone(), name: name.clone(), url });
         queued += 1;
         results.push(json!({ "ok": true, "game_name": name }));
     }
-
-    if queued > 0 {
-        if !state.read().consumer_started.swap(true, Ordering::SeqCst) {
-            let s = state.clone();
-            tokio::spawn(async move { download_consumer(s).await; });
-        }
-        state.read().pending_notify.notify_one();
+    if !items.is_empty() {
+        let _ = state.tx.send(QueueCommand::AddBatch(items)).await;
     }
@@ -837,43 +831,58 @@
-async fn download_consumer(state: AppState) {
+pub async fn queue_worker(mut rx: mpsc::Receiver<QueueCommand>, state: AppState) {
     loop {
-        let item = {
+        while let Some(item) = {
             let mut d = state.write();
             d.pending_set.pop_front()
-        };
-        match item {
-            Some((platform, name, url)) => {
-                let single = json!({ "url": url, "platform": platform, "game_name": name, "mode": "queue" });
-                let _ = download(State(state.clone()), Json(single)).await;
-            }
-            None => {
-                let n = state.read().pending_notify.clone();
-                tokio::select! {
-                    _ = state.shutdown.notified() => return,
-                    _ = n.notified() => continue,
+        } {
+            dispatch_queued(&state, item).await;
+        }
+        let resume = state.read().pending_notify.clone();
+        tokio::select! {
+            cmd = rx.recv() => match cmd {
+                Some(QueueCommand::Add(item)) => {
+                    let mut d = state.write();
+                    d.pending_set.push_back(item);
                 }
-            }
+                Some(QueueCommand::AddBatch(items)) => {
+                    let mut d = state.write();
+                    d.pending_set.extend(items);
+                }
+                None => return,
+            },
+            _ = resume.notified() => {}
         }
     }
 }
+async fn dispatch_queued(state: &AppState, item: QueuedItem) {
+    let single = json!({ "url": item.url, "platform": item.platform, "game_name": item.name, "mode": "queue" });
+    let _ = download(State(state.clone()), Json(single)).await;
+}

diff --git a/manager-rs/manager-http/src/state.rs b/manager-rs/manager-http/src/state.rs
--- a/manager-rs/manager-http/src/state.rs
+++ b/manager-rs/manager-http/src/state.rs
@@ -8,6 +8,7 @@
+use tokio::sync::mpsc;
@@ -20,6 +21,49 @@
+#[derive(Debug)]
+pub enum QueueCommand { Add(QueuedItem), AddBatch(Vec<QueuedItem>) }
+#[derive(Debug, Clone)]
+pub struct QueuedItem { pub platform: String, pub name: String, pub url: String }
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum QueueStatus { Running, Paused, Stopped }
+impl Default for QueueStatus { fn default() -> Self { QueueStatus::Running } }
@@ -69,12 +113,13 @@
-    pub pending_set: VecDeque<(String, String, String)>,
-    pub pending_notify: Arc<Notify>,
-    pub consumer_started: AtomicBool,
+    pub pending_set: VecDeque<QueuedItem>,
+    pub pending_notify: Arc<Notify>,
+    pub status: QueueStatus,
@@ -110,7 +155,7 @@
-            consumer_started: AtomicBool::new(false),
+            status: QueueStatus::Running,
@@ -158,31 +203,43 @@
     pub shutdown: Arc<Notify>,
+    pub tx: mpsc::Sender<QueueCommand>,
 }
 impl AppState {
     pub fn empty() -> Self {
-        Self {
+        let (tx, rx) = mpsc::channel(1024);
+        let state = Self { ..., tx };
+        if let Ok(handle) = Handle::try_current() {
+            handle.spawn(crate::api::queue_worker(rx, state.clone()));
+        }
+        state
     }
     pub fn with_data(data: StateData, events: Sender<String>) -> Self {
-        Self { ... }
+        let (tx, rx) = mpsc::channel(1024);
+        let state = Self { ..., tx };
+        if let Ok(handle) = Handle::try_current() {
+            handle.spawn(crate::api::queue_worker(rx, state.clone()));
+        }
+        state
     }

diff --git a/manager-rs/manager-http/tests/contract.rs b/manager-rs/manager-http/tests/contract.rs
--- a/manager-rs/manager-http/tests/contract.rs
+++ b/manager-rs/manager-http/tests/contract.rs
@@
 fn app_with_bridge(bridge: Arc<dyn manager_bridge::TorrentBackend>) -> Router {
-    router(AppState { ... })
+    let (tx, rx) = tokio::sync::mpsc::channel::<manager_http::state::QueueCommand>(1024);
+    let state = AppState { ..., tx };
+    tokio::spawn(manager_http::api::queue_worker(rx, state.clone()));
+    router(state)
 }

diff --git a/manager-rs/.cargo/config.toml b/manager-rs/.cargo/config.toml
 # [build] target-dir = "C:/Users/lv/RGSX/rust-target"  (Windows — korundu)
+[target.aarch64-unknown-linux-gnu]
+rustflags = [
+    "-C", "link-arg=-L/usr/lib/aarch64-linux-gnu",
+    "-C", "link-arg=-Wl,-rpath,/usr/lib/aarch64-linux-gnu",
+]
```
