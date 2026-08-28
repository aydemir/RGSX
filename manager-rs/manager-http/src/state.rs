//! Paylaşılan HTTP durumu: `AppState` + `StateData`.
//!
//! TASK-002b — Python `config` modülünün web yüzeyindeki karşılığı
//! (`download_queue`, `download_progress`, `history`, `downloaded_games`).
//! placeholder: gerçek persist/worker bağlantısı TASK-002c.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::{Arc, RwLock};
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio::sync::Semaphore;

use crate::catalog::CatalogSource;
use manager_bridge::TorrentBackend;
use manager_core::state::ManagerState;

use crate::sse;
use serde_json::json;
use serde_json::Value;

pub const SNAPSHOT_KEYS: [&str; 5] = ["history", "queue", "active", "progress", "downloaded"];

/// `tasks` haritasında bellekte tutulan maksimum görev sayısı (eviction cap).
/// Aşan en eski `Completed`/`Failed` görevler FIFO sırasından tahliye edilir;
/// `history` ayrı `Vec` olarak korunur (bkz. `history_path`), dolayısıyla
/// uzun süreli daemon'larda `tasks` sınırsız büyümez.
pub const TASKS_CAP: usize = 500;

/// Kuyruk worker'ına gönderilen komutlar (Faz gap-30 / F1).
///
/// `download_batch` handler'ı katalog çözümü + dedupe'u yapar ve geçerli
/// öğeleri `AddBatch` ile TEK mesajda yollar (binlerce öğe = 1 kanal mesajı).
/// Tekil `/api/download` de `Add` ile yollayabilir. Worker komutları alır,
/// `pending_set`'e yazar ve (F1'de koşulsuz, F2'de `status == Running` kapısıyla)
/// dispatch eder. `Paused`/`Stopped` durumunda kanal DİNLENMEYE DEVAM EDER —
/// gelen mesajlar DROP EDİLMEZ, yalnız buffer'a yazılır.
#[derive(Debug)]
pub enum QueueCommand {
    /// Tekil indirme isteği.
    Add(QueuedItem),
    /// Toplu indirme isteği (binlerce öğe tek mesajda).
    AddBatch(Vec<QueuedItem>),
}

/// Kuyruk worker'ının dispatch ettiği tek bir indirme öğesi.
#[derive(Debug, Clone)]
pub struct QueuedItem {
    pub platform: String,
    pub name: String,
    pub url: String,
}

/// Kuyruk görev kimliği (Faz gap-30 / F3-F4). Mevcut kodda tekil anahtar URL'dir
/// (`retry_in_flight` ile aynı anahtar → O(1) çakışmasız dedupe).
pub type TaskId = String;

/// Bir görevin kuyruk içi durumu (Faz gap-30 / F3-F4). O(1) durum sorgusu
/// (`is_queued` / `get_status`) için `tasks: HashMap<TaskId, TaskState>` kullanılır;
/// böylece liste render / scan sırasında `Vec` taraması (O(N)) ortadan kalkar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Queued,
    Active,
    Completed,
    Failed,
}

/// Kuyruk durum makinesi (Faz gap-30 / F1).
///
/// `Stopped` şu an kullanımda değil (worker yalnız `rx.recv() == None`, yani
/// tüm `tx` drop edilince biter). `Paused` kapısı F2'de devreye girer; o zaman
/// dispatch `status == Running` ile gate'lenir ve `Paused→Running` geçişi
/// worker'ı `pending_notify` ile uyandırır.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueStatus {
    Running,
    Paused,
    Stopped,
}

impl Default for QueueStatus {
    fn default() -> Self {
        QueueStatus::Running
    }
}

/// İş mantığı durumu (config eşleniği). `RwLock` + senkron erişim — axum
/// handler'ları kısa tutulduğu için `tokio::sync`'e gerek yok.
#[derive(Debug)]
pub struct StateData {
    /// `config.history` — indirme geçmişi.
    pub history: Vec<serde_json::Value>,
    /// `config.download_queue` — kuyruktaki görevler.
    pub queue: Vec<serde_json::Value>,
    /// `config.download_progress` — ilerleme haritası (url -> yüzde/bytes).
    pub progress: serde_json::Value,
    /// `config.downloaded_games` — tamamlanan oyunlar.
    pub downloaded: serde_json::Value,
    /// TASK-002-gap-10 (A): history.json diske yazılacağı yol. `None` ise
    /// kalıcılık kapalı (test modu / env set değil). Startup'ta `main.rs` tarafından set edilir.
    pub history_path: Option<std::path::PathBuf>,
    /// `config.download_active` — aktif indirme bayrağı.
    pub active: bool,
    /// Manager durumu (watchdog state machine; varsayılan INIT).
    pub manager_state: ManagerState,
    /// Process id — `/api/health` `pid` alanı.
    pub pid: u32,
    /// `/api/settings` dönen app ayarları (placeholder).
    pub settings: serde_json::Value,
    /// `/api/system_info` — cihaz bilgileri (placeholder).
    pub system_info: serde_json::Value,
    /// TASK-002-gap-1: URL başına retry sayacı (failures). Key = `game_url`.
    pub retries: HashMap<String, u32>,
    /// TASK-002-gap-1: devam eden retry dizisini deduplike eden set. Key = `game_url`.
    pub retry_in_flight: HashSet<String>,
    /// TASK-002-gap-1: URL başına son `retry_at` (unix epoch saniye). Key = `game_url`.
    pub retry_at: HashMap<String, f64>,
    /// TASK-002-gap-1: URL başına iptal sinyali (retry sleep'ini kesmek için).
    /// Key = `game_url` (veya cancel'da verilen `task_id`).
    pub cancel_signals: HashMap<String, Arc<Notify>>,
    /// Faz 12.6d — eşzamanlı indirme sayısını sınırlayan semaphore. Kapasite
    /// `Settings.max_simultaneous_downloads`'tan türetilir (startup + ayar kaydı).
    pub download_semaphore: Arc<Semaphore>,
    /// `download_semaphore` kapasitesi (stop-all sonrası yeniden oluşturmak için saklanır).
    pub max_simultaneous_downloads: usize,
    /// TASK-002-gap-12: "stop all" (kuyruk temizle + tüm indirmeleri iptal) bayrağı.
    /// `queue_clear` set eder; semaphore'da bekleyen/indiren görevler bunu kontrol
    /// edip çıkar. Kısa bir gecikmeyle (bkz. queue_clear) sıfırlanır.
    pub aborting: AtomicBool,
    /// Faz 12 "Download All" refactor: toplu enqueue artık handler içinde
    /// yapılmaz. `download_batch` yalnızca geçerli öğeleri buraya `push_back`
    /// eder (O(1) dönüş) ve tek bir arka plan `download_consumer` döngüsü bu
    /// Kuyruk sıra takibi (Faz gap-30 / F3): FIFO sıra = `VecDeque<TaskId>`
    /// (TaskId = url). Gerçek payload `queued_items` haritasındadır; pop_front
    /// bir TaskId verir, payload oradan O(1) alınır.
    pub pending_set: VecDeque<TaskId>,
    /// `pending_set` içindeki TaskId → öğe payload'ı (O(1) lookup).
    pub queued_items: HashMap<TaskId, QueuedItem>,
    /// O(1) durum sorgusu: TaskId → `TaskState` (Queued/Active/Completed/Failed).
    /// Liste render / scan `Vec` taraması yapmaz, bunu kullanır.
    pub tasks: HashMap<TaskId, TaskState>,
    /// `tasks` eviction FIFO sırası: tamamlanan görevlerin tahliye edileceği sıra.
    /// Yalnız `Completed`/`Failed` geçişlerinde doldurulur; in-flight görevler
    /// (Queued/Active) buraya YAZILMAZ, böylece yanlışlıkla tahliye edilmezler.
    pub tasks_order: VecDeque<TaskId>,
    /// O(1) "queued?" üyelik seti. `download_batch` dedupe ve UI snapshot'ı bunu
    /// kullanır (kilit altında mikrosaniyede clone edilir).
    pub queued_ids: HashSet<TaskId>,
    /// O(1) "already downloaded?" indeksi: `(platform, name)` çiftleri. `downloaded`
    /// `Value` array'inin O(N) taraması yerine geçer (finalize'da artırımlı doldurulur).
    pub downloaded_index: HashSet<(String, String)>,
    /// `queue_worker`'ı uyandırmak için notify (Arc çünkü borrow süresi
    /// select içinde guard'dan uzun olabilir). F2'de `Paused→Running`
    /// geçişinde `notify_waiters()` ile dispatch döngüsü uyandırılır.
    pub pending_notify: Arc<Notify>,
    /// Kuyruk durum makinesi (F1: `Running`; F2'de `Paused`/`Stopped` kapısı).
    pub status: QueueStatus,
    /// TASK-002-gap-29: resume sinyali — duraklatılmış indirme döngüleri bununla
    /// uyandırılır (`global_paused` false olduktan sonra `notify_all`).
    pub pause_resume: Arc<Notify>,
    /// TASK-002-gap-29: URL başına pause sinyali. Global pause'da devam eden
    /// native HTTP-direct indirmelerin `CancelFlag`'i tetiklenir (abort).
    pub pause_signals: HashMap<String, Arc<Notify>>,
    /// TASK-002-gap-32: ağ bağlantısı koptuğunda indirmeleri PARK eden global bayrak.
    /// `true` iken tüm indirme döngüleri duraklatılır (retry budget yakılmaz); bağlantı
    /// geri gelince `network_resume` ile uyandırılıp kaldığı yerden devam eder.
    /// `StateData` içinde tutulur (global_paused gibi dış AppState değil) çünkü hem
    /// park gate (okuma) hem decide_retry/reconnect-probe (yazma) zaten kilitliyken
    /// erişir; ayrıca SSE snapshot'ına `network_down` alanı olarak kolayca eklenir.
    pub network_down: Arc<AtomicBool>,
    /// Katalog OTA bootstrap tamamlandı mı? (0/1 yerine bool — snapshot'ta TVUI'ye sinyal)
    pub catalog_ready: Arc<AtomicBool>,
    /// Bootstrap başarısızsa hata nedeni (snapshot'ta TVUI'ye düşer).
    pub catalog_error: Option<String>,
    /// TASK-012m — manager self-update mevcutsa `{available,version,url,sha256,stage,...}` (snapshot'ta TVUI'ye).
    pub manager_update: Option<serde_json::Value>,
    /// TASK-012m Faz 5 — self-update indirmesi iptal sinyali (kuyruktan iptal ile set edilir).
    pub manager_update_cancel: Arc<AtomicBool>,
    /// `network_down`→`false` geçişinde (yeniden bağlanınca) bekleyen döngüleri uyandırır.
    pub network_resume: Arc<Notify>,
    /// Ardışık Network hatası sayacı — `NETWORK_DOWN_THRESHOLD`'a ulaşınca `network_down`
    /// `true` yapılır. Ağ tekrar dönünce veya Network-dışı hata olunca sıfırlanır.
    pub network_error_streak: Arc<AtomicU32>,
    /// TASK-002-gap-32: GERÇEK kesinti onayı. Park gate'de probe BAŞARISIZ olunca
    /// (`network_down` zaten `true`) `true` set edilir; probe başarılı olup `network_down`
    /// `false`'a çekilirken yalnızca bu bayrak `true` ise `network_restored` SSE olayı
    /// yayınlanır (sonra temizlenir). Böylece tek bir ölü host (internet yukarı) titreşimi
    /// sahte "bağlantı geri geldi" bildirimi üretmez — bayrak yalnız gerçek outage'da set edilir.
    pub network_outage_confirmed: Arc<AtomicBool>,
}

impl StateData {
    /// Boş state:`/api/health` için `pid > 0` garanti eder (contract testi).
    pub fn empty() -> Self {
        Self {
            history: vec![],
            queue: vec![],
            progress: serde_json::json!({}),
            downloaded: serde_json::json!({}),
            history_path: None,
            active: false,
            manager_state: ManagerState::Init,
            pid: std::process::id(),
            settings: serde_json::json!({}),
            system_info: serde_json::json!({}),
            retries: HashMap::new(),
            retry_in_flight: HashSet::new(),
            retry_at: HashMap::new(),
            cancel_signals: HashMap::new(),
            download_semaphore: Arc::new(Semaphore::new(5)),
            max_simultaneous_downloads: 5,
            aborting: AtomicBool::new(false),
            pending_set: VecDeque::new(),
            queued_items: HashMap::new(),
            tasks: HashMap::new(),
            tasks_order: VecDeque::new(),
            queued_ids: HashSet::new(),
            downloaded_index: HashSet::new(),
            pending_notify: Arc::new(Notify::new()),
            status: QueueStatus::Running,
            pause_resume: Arc::new(Notify::new()),
            pause_signals: HashMap::new(),
            network_down: Arc::new(AtomicBool::new(false)),
            catalog_ready: Arc::new(AtomicBool::new(false)),
            catalog_error: None,
            manager_update: None,
            manager_update_cancel: Arc::new(AtomicBool::new(false)),
            network_resume: Arc::new(Notify::new()),
            network_error_streak: Arc::new(AtomicU32::new(0)),
            network_outage_confirmed: Arc::new(AtomicBool::new(false)),
        }
    }

    /// `config.download_queue` uzunluğu.
    pub fn queue_size(&self) -> usize {
        self.queue.len()
    }

    /// `tasks` haritasına durum yazar ve eviction FIFO sırasını günceller.
    /// Yalnız `Completed`/`Failed` geçişlerinde sıraya eklenir (in-flight görevler
    /// korunur); yazma sonrası `TASKS_CAP` altına çekmek için tahliye yapılır.
    pub fn set_task_state(&mut self, id: TaskId, state: TaskState) {
        let is_terminal = matches!(state, TaskState::Completed | TaskState::Failed);
        self.tasks.insert(id.clone(), state);
        if is_terminal && !self.tasks_order.contains(&id) {
            self.tasks_order.push_back(id);
        }
        self.evict_tasks(TASKS_CAP);
    }

    /// `tasks` haritasını `TASKS_CAP` altında tutar: `tasks_order` başından
    /// tarayarak en eski `Completed`/`Failed` görevleri tahliye eder. Baştaki
    /// giriş in-flight (Queued/Active) ise tarama durur — aktif görev silinmez.
    pub fn evict_tasks(&mut self, cap: usize) {
        while self.tasks.len() > cap {
            let front = match self.tasks_order.front() {
                Some(f) => f.clone(),
                None => break,
            };
            match self.tasks.get(&front) {
                Some(TaskState::Completed) | Some(TaskState::Failed) => {
                    self.tasks_order.pop_front();
                    self.tasks.remove(&front);
                }
                _ => break,
            }
        }
    }

    /// `stop all` / kuyruk temizleme: `tasks` ve eviction sırası birlikte sıfırlanır.
    pub fn clear_tasks(&mut self) {
        self.tasks.clear();
        self.tasks_order.clear();
    }

    /// `downloaded` `Value` (platform→[name]) yapısından O(1) `downloaded_index`
    /// haritasını (yeniden) türetir. `main.rs` startup ve settings tazelemesinde
    /// `downloaded` değiştiğinde çağrılır; böylece `download_batch` O(N) array
    /// taraması yapmaz, doğrudan `downloaded_index.contains` ile O(1) bakar.
    pub fn rebuild_downloaded_index(&mut self) {
        let mut idx = HashSet::new();
        if let Value::Object(map) = &self.downloaded {
            for (platform, arr) in map {
                if let Some(arr) = arr.as_array() {
                    for g in arr {
                        if let Some(name) = g.as_str() {
                            idx.insert((platform.clone(), name.to_string()));
                        }
                    }
                }
            }
        }
        self.downloaded_index = idx;
    }

    /// TASK-002-gap-9 (M0) — restart sonrası yarıda kalan indirmeyi sürdürme.
    ///
    /// Python `_resume_interrupted_downloads` (rgsx_manager.py:743) parity'si:
    /// `history`'de `Downloading`/`Téléchargement`/`Paused` status'lu entry'ler
    /// varsa bunları `Queued`'a çevirip kuyruğa geri ekler. qBittorrent path'inde
    /// partial verisi korunur → kaldığı yerden devam; Rust librqbit path'inde de
    /// `.rqbitpart` diskte kalır, yeniden `add_torrent(overwrite=true)` ile resume
    /// edilir (HTTP ise `Range` resume retry envelope içinde).
    ///
    /// - `already downloaded` (downloaded_index) veya hâlâ `Queued`/`Active` ise atlanır.
    /// - `url` yoksa veya boşsa atlanır.
    /// Döner: yeniden kuyruğa alınan entry sayısı.
    pub fn resume_interrupted_downloads(&mut self) -> usize {
        let mut to_enqueue: Vec<(usize, String, String, String)> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        for (idx, entry) in self.history.iter().enumerate() {
            let Some(obj) = entry.as_object() else { continue };
            let status = obj
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !is_interrupted_status(status) {
                continue;
            }
            let url = obj
                .get("url")
                .or_else(|| obj.get("source_url"))
                .or_else(|| obj.get("download_url"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if url.is_empty() {
                continue;
            }
            let platform = obj
                .get("platform")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let name = obj
                .get("game_name")
                .or_else(|| obj.get("name"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            // already downloaded → skip (O(1) index)
            if !platform.is_empty() && !name.is_empty() && self.downloaded_index.contains(&(platform.clone(), name.clone())) {
                continue;
            }
            // already queued/active → skip
            if self.queued_ids.contains(&url) {
                continue;
            }
            if let Some(st) = self.tasks.get(&url) {
                if matches!(st, TaskState::Queued | TaskState::Active) {
                    continue;
                }
            }
            if seen.contains(&url) {
                continue;
            }
            seen.insert(url.clone());
            // also check if any history already queued for same url (covers pre-rebuild)
            to_enqueue.push((idx, url, platform, name));
        }
        let mut n = 0usize;
        for (idx, url, platform, name) in to_enqueue {
            // history entry status → Queued
            if let Some(entry) = self.history.get_mut(idx) {
                if let Some(obj) = entry.as_object_mut() {
                    obj.insert("status".to_string(), json!("Queued"));
                    obj.insert("entity_state".to_string(), json!("QUEUED"));
                }
            }
            // progress → Queued 0%
            self.progress[&url] = json!({ "status": "Queued", "progress": 0 });
            // enqueue only if we have at least url + name/platform (fallback name from url)
            let display_name = if name.is_empty() {
                url.rsplit('/').next().unwrap_or(&url).to_string()
            } else {
                name
            };
            let plat = if platform.is_empty() { "unknown".to_string() } else { platform };
            let item = QueuedItem {
                platform: plat,
                name: display_name,
                url: url.clone(),
            };
            // gap-30 queue structures
            if !self.queued_items.contains_key(&url) {
                self.pending_set.push_back(url.clone());
                self.queued_items.insert(url.clone(), item);
            }
            self.tasks.insert(url.clone(), TaskState::Queued);
            self.queued_ids.insert(url.clone());
            // legacy queue Vec parity (api.rs queue_size uses Vec, but new system uses pending_set;
            // keep Vec in sync for SSE snapshot backwards compat)
            // Avoid duplicating Vec entry if already present
            if !self.queue.iter().any(|v| v.get("url").and_then(Value::as_str) == Some(&url)) {
                self.queue.push(json!({
                    "url": url,
                    "platform": self.queued_items.get(&url).map(|i| i.platform.clone()).unwrap_or_default(),
                    "game_name": self.queued_items.get(&url).map(|i| i.name.clone()).unwrap_or_default(),
                    "status": "Queued",
                }));
            }
            n += 1;
        }
        if n > 0 {
            tracing::info!("resume_interrupted: {} entry Queued'a alındı", n);
        }
        n
    }

    /// `/api/browse-directories` — verilen kökün alt dizinleri (placeholder: boş).
    /// Python `handlers_ui.py` gerçek tarama yapar; bu slice sadece şablon.
    pub fn browse(&self, path: &str) -> (String, Vec<serde_json::Value>) {
        let dirs: Vec<serde_json::Value> = std::fs::read_dir(path)
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.path().is_dir())
                    .map(|e| {
                        json!({
                            "name": e.file_name().to_string_lossy().to_string(),
                            "path": e.path().to_string_lossy().to_string(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        (path.to_string(), dirs)
    }
}

/// M0 helper — `history` status interrupted sayılır mı?
fn is_interrupted_status(s: &str) -> bool {
    let lower = s.trim().to_lowercase();
    lower == "downloading"
        || lower == "paused"
        || lower.starts_with("téléchargement")
        || lower.starts_with("telechargement")
}

/// Axum handler'lara `State` extractor ile verilen paylaşılan durum.
#[derive(Clone)]
pub struct AppState {
    pub data: Arc<RwLock<StateData>>,
    pub events: Sender<String>,
    /// Torrent backend (Python bridge subprocess veya in-process librqbit engine).
    /// manager-bin kurar; handler'lar buradan `call` yapar.
    pub bridge: Option<Arc<dyn TorrentBackend>>,
    /// WebUI statik dosyalarının kökü (`.../static/`). None ise statik servis kapalı.
    pub static_root: Option<std::path::PathBuf>,
    /// Faz 10c/3/2 — katalog proxy kaynağı (`CatalogSource`). None ise handler'lar
    /// placeholder'a düşer (geriye uyumlu).
    pub catalog: Option<Arc<dyn CatalogSource>>,
    /// TASK-002-gap-1: global shutdown sinyali (retry sleep'lerini keser).
    pub shutdown: Arc<Notify>,
    /// Faz gap-30 / F1: kuyruk worker'ına komut kanalı (clone'lanabilir sender).
    /// `AppState::empty()`/`with_data()` worker'ı `tokio::spawn` ile başlatır.
    pub tx: mpsc::Sender<QueueCommand>,
    /// TASK-002-gap-29: global pause bayrağı (native HTTP-direct mod). RwLock
    /// dışında `AtomicBool` — saniyede yüzlerce kez okunsa bile zero-lock O(1)
    /// (CPU cache invalidation sürtünmesi yok). `true` iken yeni indirme başlamaz.
    pub global_paused: Arc<AtomicBool>,
    /// SSE yayını için "değişti mi?" bayrağı (F6 optimization). Durum her değiştiğinde
    /// `store(true)` yapılır; `broadcast_loop` yalnızca `true` iken serileştirip yayınlar
    /// (idle daemon'da gereksiz JSON serialization overhead'i önlenir). 30s tam-snapshot
    /// yayını bu bayraktan BAĞIMSIZDIR — böylece bir set atlanırsa en fazla 30s bayatlar,
    /// kalıcı SSE uyumsuzluğu oluşmaz.
    pub dirty: Arc<AtomicBool>,
}

impl AppState {
    /// Boş state + yeni SSE kanalı. Kuyruk worker'ı da burada `tokio::spawn`
    /// ile başlatılır (runtime içinde çağrılmalı — handler/test bağlamı).
    pub fn empty() -> Self {
        let (tx, rx) = mpsc::channel(1024);
        let state = Self {
            data: Arc::new(RwLock::new(StateData::empty())),
            events: sse::channel(),
            bridge: None,
            static_root: None,
            catalog: None,
            shutdown: Arc::new(Notify::new()),
            tx,
            global_paused: Arc::new(AtomicBool::new(false)),
            dirty: Arc::new(AtomicBool::new(true)),
        };
        tokio::spawn(crate::api::queue_worker(rx, state.clone()));
        tokio::spawn(crate::sse::broadcast_loop(state.clone()));
        state
    }

    /// Kanalı paylaşırken (test) verilen sender ile kurar. Worker yine burada spawn edilir.
    pub fn with_data(data: StateData, events: Sender<String>) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        let state = Self {
            data: Arc::new(RwLock::new(data)),
            events,
            bridge: None,
            static_root: None,
            catalog: None,
            shutdown: Arc::new(Notify::new()),
            tx,
            global_paused: Arc::new(AtomicBool::new(false)),
            dirty: Arc::new(AtomicBool::new(true)),
        };
        tokio::spawn(crate::api::queue_worker(rx, state.clone()));
        tokio::spawn(crate::sse::broadcast_loop(state.clone()));
        state
    }

    /// bridge yoksa sahte `BridgeError::Spawn` döndürür (handler'lar placeholder
    /// davranışına düşer). Varsa `call`'ı proxy eder.
    pub async fn bridge_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, manager_bridge::BridgeError> {
        match &self.bridge {
            Some(b) => b.call(method, params).await,
            None => Err(manager_bridge::BridgeError::Spawn(
                "bridge başlatılmadı".to_string(),
            )),
        }
    }

    /// Okuma kilidi (poison'u unwrap — placeholder; Python config benzeri global).
    pub fn read(&self) -> std::sync::RwLockReadGuard<'_, StateData> {
        self.data.read().unwrap()
    }

    /// Yazma kilidi (poison'u unwrap).
    pub fn write(&self) -> std::sync::RwLockWriteGuard<'_, StateData> {
        self.data.write().unwrap()
    }

    /// Faz gap-30 / F3-F4: O(1) "queued?" üyelik snapshot'ı. Kilit yalnızca
    /// `HashSet` clone'ı kadar (mikrosaniye) tutulur; liste render / scan bunu
    /// kullanarak worker kilitlerini uzun süre tutmaz. `Vec` taraması YOK.
    pub fn queued_ids_snapshot(&self) -> HashSet<TaskId> {
        self.read().queued_ids.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resume_interrupted_queues_paused_downloading() {
        let mut data = StateData::empty();
        data.history = vec![
            json!({"platform":"psx","game_name":"GameA","url":"https://a.com/a.zip","status":"Downloading"}),
            json!({"platform":"snes","game_name":"GameB","url":"https://b.com/b.zip","status":"Téléchargement"}),
            json!({"platform":"nes","game_name":"GameC","url":"https://c.com/c.zip","status":"Paused"}),
            json!({"platform":"psx","game_name":"GameD","url":"https://d.com/d.zip","status":"Download_OK"}),
            json!({"platform":"psx","game_name":"GameE","url":"https://e.com/e.zip","status":"Queued"}),
            json!({"platform":"psx","game_name":"GameF","url":"https://f.com/f.zip","status":"Erreur"}),
        ];
        data.rebuild_downloaded_index();
        let n = data.resume_interrupted_downloads();
        assert_eq!(n, 3, "Downloading/Téléchargement/Paused → 3 Queued");
        assert_eq!(data.pending_set.len(), 3);
        assert_eq!(data.queued_ids.len(), 3);
        assert!(data.queued_ids.contains("https://a.com/a.zip"));
        assert!(data.queued_ids.contains("https://b.com/b.zip"));
        assert!(data.queued_ids.contains("https://c.com/c.zip"));
        // history status mutated to Queued
        assert_eq!(data.history[0]["status"], json!("Queued"));
        assert_eq!(data.history[1]["status"], json!("Queued"));
        assert_eq!(data.history[2]["status"], json!("Queued"));
        // non-interrupted untouched
        assert_eq!(data.history[3]["status"], json!("Download_OK"));
        assert_eq!(data.history[4]["status"], json!("Queued"));
        // progress seeded
        assert_eq!(data.progress["https://a.com/a.zip"]["status"], json!("Queued"));
        // tasks
        assert_eq!(data.tasks.get("https://a.com/a.zip"), Some(&TaskState::Queued));
    }

    #[test]
    fn resume_does_not_requeue_already_downloaded() {
        let mut data = StateData::empty();
        data.downloaded = json!({"psx":["GameA"]});
        data.rebuild_downloaded_index();
        data.history = vec![
            json!({"platform":"psx","game_name":"GameA","url":"https://a.com/a.zip","status":"Downloading"}),
            json!({"platform":"psx","game_name":"GameB","url":"https://b.com/b.zip","status":"Downloading"}),
        ];
        let n = data.resume_interrupted_downloads();
        assert_eq!(n, 1, "already downloaded skip");
        assert!(data.queued_ids.contains("https://b.com/b.zip"));
        assert!(!data.queued_ids.contains("https://a.com/a.zip"));
    }

    #[test]
    fn resume_skips_already_queued_and_empty_url() {
        let mut data = StateData::empty();
        data.history = vec![
            json!({"platform":"psx","game_name":"GameA","url":"https://a.com/a.zip","status":"Downloading"}),
            json!({"platform":"psx","game_name":"GameA","url":"https://a.com/a.zip","status":"Paused"}),
            json!({"platform":"psx","game_name":"GameX","status":"Downloading"}),
        ];
        let n = data.resume_interrupted_downloads();
        assert_eq!(n, 1, "duplicate url only once, empty url skip");
        assert_eq!(data.pending_set.len(), 1);
    }

    #[test]
    fn resume_tolerates_telechargement_variants() {
        let mut data = StateData::empty();
        data.history = vec![
            json!({"platform":"psx","game_name":"G1","url":"https://1.com/1.zip","status":"Téléchargement en cours"}),
            json!({"platform":"psx","game_name":"G2","url":"https://2.com/2.zip","status":"téléchargement"}),
            json!({"platform":"psx","game_name":"G3","url":"https://3.com/3.zip","status":"DOWNLOADING"}),
        ];
        let n = data.resume_interrupted_downloads();
        assert_eq!(n, 3);
    }
}
