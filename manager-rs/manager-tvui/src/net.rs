//! TASK-012h Faz 2c - TVUI tarafı SSE istemcisi (senkron, ureq).
//!
//! manager-http `/api/events` akisini dinler; `catalog_update` olaylarini
//! paylasilan `TvuiState`'e yazar. SDL2 dongusu bunu okuyup loading bar'ini cizer.
//! Senkron olmasi bilincli: SDL2 event loop tek thread, async/tokio agirligi gereksiz.

use std::io::BufRead;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Self-update indirmesinin kuyruk görevi kimliği (manager-http ile aynı sabit).
pub const MANAGER_UPDATE_TASK_ID: &str = "manager-update";

/// SSE okuma watchdog'u: sunucu 30 sn'de bir keep-alive snapshot yayınlar
/// (`manager-http/src/sse.rs` broadcast_loop); 90 sn sessizlik ≥2 kaçırılmış
/// keep-alive demektir → bağlantı ölü sayılır, reconnect döngüsü devreye girer.
const SSE_READ_TIMEOUT: Duration = Duration::from_secs(90);

/// Yeniden bağlanma gecikmesi (Python parity: tvui.py `_manager_sse_worker` 3 sn).
const SSE_RECONNECT_DELAY: Duration = Duration::from_secs(3);

/// Faz B (bulgu 7): apply sonrası relaunch normalde süreci anında devirir; bu
/// sürede gelmediyse overlay kapanıp banner `failed`'e döner.
pub const RESTART_OVERLAY_TIMEOUT: Duration = Duration::from_secs(60);

/// Poison-safe kilit: bir thread lock tutarken paniklese bile diğer thread
/// domino gibi düşmesin (Python'daki her şeyi-saran try/except'in karşılığı).
pub(crate) fn tvui_lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Kullanıcı tetiklemeli API çağrıları için ortak agent: connect 3s + toplam 5s.
/// SDL event loop'undan çağrıldıkları için donma üst sınırı zorunludur
/// (TASK-012-gap-01 bulgu 3).
fn api_agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .build()
}

/// TVUI acilis durumu (loading bar kaynagi). SDL2 dongusu ile SSE thread'i
/// arasinda `Arc<Mutex<>>` ile paylasilir.
#[derive(Debug, Clone, Default)]
pub struct TvuiState {
    pub loading: bool,
    pub pct: i64,
    pub stage: String,
    pub ready: bool,
    pub error: Option<String>,
    /// `ready` olunca `/api/platforms`'tan çekilen platformlar (grid kaynağı).
    pub platforms: Vec<PlatformTile>,
    /// TASK-012m — manager self-update mevcutsa versiyon (placeholder prompt için).
    pub update_available: Option<String>,
    /// TASK-012m Faz 5 — self-update akış aşaması:
    /// `available` → `downloading` → `ready` → `applying` → (`yeniden başlatma`).
    /// Hata/iptalde `failed` / `available`'a döner.
    pub update_stage: Option<String>,
    /// İndirme yüzdesi (0-100), `downloading` aşamasında banner'da gösterilir.
    pub update_pct: u32,
    /// Apply sonrası "Yeniden başlatılıyor…" ekranı için bayrak.
    pub update_restarting: bool,
    /// `update_restarting`'in başladığı an (Faz B, bulgu 7): relaunch bu sürede
    /// süreci devralmazsa overlay otomatik kapanır — ölü ekranda kilitlenme yok.
    pub update_restarting_since: Option<Instant>,
    /// SSE/HTTP bağlantı portu (Enter→download tetiklemede kullanılır).
    pub port: u16,
    /// TASK: bootstrap fail sonrası kullanıcı "çevrimdışı devam" seçtiyse true
    /// (grid boş kategoriyle, kırmızı şeritle işaretli gösterilir).
    pub offline: bool,
}

/// Tek bir platform kutusu (grid tile'ı). `name` görünen etiket, `folder` disk
/// eşleşmesi (sonraki faz: game_list).
#[derive(Debug, Clone, Default)]
pub struct PlatformTile {
    pub name: String,
    pub folder: String,
}

pub type SharedTvuiState = Arc<Mutex<TvuiState>>;

/// Tek bir SSE cercevesini ayristirir: `event: <type>\ndata: <json>\n\n` bloklarindan
/// `(event_type, json)` dondurur. `event:`/`data:` satirlari olmadan `None`.
/// Çok satırlı `data:` satırları `\n` ile birleştirilir (SSE spec + Python parity,
/// tvui.py `_stream_sse`) — bulgu 14.
pub fn parse_sse_frame(buf: &str) -> Option<(String, serde_json::Value)> {
    let mut event = String::new();
    let mut data = String::new();
    for line in buf.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.trim());
        }
    }
    if event.is_empty() || data.is_empty() {
        return None;
    }
    serde_json::from_str(&data).ok().map(|v| (event, v))
}

fn apply_catalog_update(state: &SharedTvuiState, data: &serde_json::Value) {
    let mut s = tvui_lock(state);
    s.loading = true;
    if let Some(stage) = data.get("stage").and_then(|v| v.as_str()) {
        s.stage = stage.to_string();
        if stage == "ready" {
            let ok = data.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            s.loading = false;
            if ok {
                // Başarı: hazır say, hata temizle.
                s.ready = true;
                s.pct = 100;
                s.error = None;
                s.offline = false;
            } else {
                // Başarısızlık: hazır SAYMA (eski davranış boş grid'e atlıyordu).
                // Hata ekranı kalır; kullanıcı retry / offline devam karar verir.
                s.ready = false;
                let reason = data
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("bilinmiyor");
                s.error = Some(format!("katalog hazirlanamadi: {reason}"));
            }
        }
    }
    if let Some(pct) = data.get("pct").and_then(|v| v.as_i64()) {
        s.pct = pct;
    }
}

/// Başlangıç `snapshot` olayını işler (race düzeltmesi): `catalog_ready` true ise
/// TVUI loading bar'ını kapatır — `catalog_update` kaçırılsa bile geç abone kurtulur.
/// Katalog hazirse eski hata/çevrimdışı bayrakları bayatmıştır: temizlenir
/// (TASK-012-gap-01 Faz A; SSE kopma-kopma geri gelme sonrası eski hata asılı kalmaz).
fn apply_snapshot(state: &SharedTvuiState, data: &serde_json::Value) {
    if let Some(true) = data.get("catalog_ready").and_then(|v| v.as_bool()) {
        let mut s = tvui_lock(state);
        s.ready = true;
        s.loading = false;
        s.pct = 100;
        s.error = None;
        s.offline = false;
    }
}

/// `/api/platforms` yanıtını (`{platforms:[{platform_name,folder,...}]}`) tile listesine çözer.
pub fn parse_platforms(v: &serde_json::Value) -> Vec<PlatformTile> {
    v.get("platforms")
        .and_then(|a| a.as_array())
        .map(|arr| {
            arr.iter()
                .map(|p| PlatformTile {
                    name: p
                        .get("platform_name")
                        .or_else(|| p.get("name"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                    folder: p
                        .get("folder")
                        .or_else(|| p.get("dossier"))
                        .and_then(|x| x.as_str())
                        .unwrap_or("")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `ready` olunca bir kez `/api/platforms`'ı çeker (grid kaynağı). Hata/boş → boş liste.
fn fetch_platforms(port: u16) -> Vec<PlatformTile> {
    let url = format!("http://127.0.0.1:{port}/api/platforms");
    match api_agent().get(&url).call() {
        Ok(r) => r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| parse_platforms(&v))
            .unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

// ===== TASK-012-gap-01 Faz B — SDL'siz UI karar katmanı (bulgu 15) =====

/// Shell'in üretebileceği yüksek seviye aksiyonlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiAction {
    RetryCatalog,
    ContinueOffline,
    UpdateDownload,
    UpdateApply,
    UpdateCancel,
}

/// Fiziksel tuşların (`Keycode`) shell tarafından çevrildiği semantik tuşlar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiKey {
    Retry,
    Confirm,
    CancelUpdate,
}

/// SAF karar fonksiyonu: mevcut durum + semantik tuş → yapılacak aksiyon.
/// HTTP ve SDL içermez; tüm geçiş kuralları burada toplanır ve unit-test edilir.
pub fn ui_decision(s: &TvuiState, key: UiKey) -> Option<UiAction> {
    match key {
        UiKey::Retry => {
            if s.error.is_some() {
                Some(UiAction::RetryCatalog)
            } else {
                None
            }
        }
        UiKey::CancelUpdate => {
            if s.update_stage.as_deref() == Some("downloading") {
                Some(UiAction::UpdateCancel)
            } else {
                None
            }
        }
        UiKey::Confirm => {
            if s.update_restarting {
                None // Zaten yeniden başlatılıyor; ikinci Enter yok sayılır.
            } else if s.error.is_some() && !s.offline {
                Some(UiAction::ContinueOffline)
            } else if s.update_available.is_some() {
                match s.update_stage.as_deref().unwrap_or("available") {
                    "ready" => Some(UiAction::UpdateApply),
                    _ => Some(UiAction::UpdateDownload),
                }
            } else {
                None
            }
        }
    }
}

/// Aksiyonu uygular: yerel state mutasyonları senkron, HTTP çağrıları ARKA PLAN
/// thread'inde (bulgu 3 — SDL event loop'u asla ağ çağrısıyla bloklanmaz).
pub fn apply_ui_action(state: &SharedTvuiState, action: UiAction) {
    match action {
        UiAction::ContinueOffline => {
            tvui_lock(state).offline = true;
        }
        UiAction::RetryCatalog => {
            {
                let mut s = tvui_lock(state);
                // Yerel durumu sıfırla; ilerleme SSE `catalog_update` ile gelir.
                s.loading = true;
                s.ready = false;
                s.error = None;
                s.pct = 0;
                s.offline = false;
            }
            let st = Arc::clone(state);
            std::thread::spawn(move || {
                let port = tvui_lock(&st).port;
                let r = trigger_catalog_retry(port);
                eprintln!("TVUI katalog retry: {}", r.message);
            });
        }
        UiAction::UpdateDownload => {
            let st = Arc::clone(state);
            std::thread::spawn(move || {
                let port = tvui_lock(&st).port;
                let r = trigger_update_download(port);
                let mut s = tvui_lock(&st);
                if r.ok {
                    // Bulgu 6: stage yalnızca istek gerçekten kabul edildiyse set edilir.
                    s.update_stage = Some("downloading".to_string());
                } else {
                    s.update_stage = Some("failed".to_string());
                }
                eprintln!("TVUI güncelleme indirme: {}", r.message);
            });
        }
        UiAction::UpdateApply => {
            let st = Arc::clone(state);
            std::thread::spawn(move || {
                let port = tvui_lock(&st).port;
                let r = trigger_update_apply(port);
                let mut s = tvui_lock(&st);
                if r.ok {
                    s.update_restarting = true;
                    s.update_restarting_since = Some(Instant::now());
                } else {
                    s.update_stage = Some("failed".to_string());
                }
                eprintln!("TVUI güncelleme apply: {}", r.message);
            });
        }
        UiAction::UpdateCancel => {
            let st = Arc::clone(state);
            std::thread::spawn(move || {
                let port = tvui_lock(&st).port;
                let r = trigger_update_cancel(port);
                eprintln!("TVUI güncelleme iptal: {}", r.message);
            });
        }
    }
}

/// Bulgu 7 — overlay takılma koruması: `RESTART_OVERLAY_TIMEOUT` içinde relaunch
/// gelmediyse overlay kapanır ve banner `failed`'e döner. Her frame'de shell
/// çağırır; saf olduğu için unit-test edilir.
pub fn expire_stale_restart_at(s: &mut TvuiState, now: Instant) -> bool {
    let expired = s.update_restarting
        && s.update_restarting_since
            .map(|t| now.duration_since(t) >= RESTART_OVERLAY_TIMEOUT)
            .unwrap_or(true); // since kayıpsa da güvenli tarafta kal (overlay kapat).
    if expired {
        s.update_restarting = false;
        s.update_restarting_since = None;
        s.update_stage = Some("failed".to_string());
    }
    expired
}

// ===== Faz C (bulgu 9) — SSE `gamepad` → SDL shell köprüsü =====

/// Gamepad olayının shell'e ilettiği niyet: semantik tuş ya da shell'den çıkış.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadIntent {
    Key(UiKey),
    Exit,
}

/// SSE `gamepad` olayının `action` alanını niyete çevirir (sözleşme:
/// `native_input::RgsxAction` → SSE stringleri, App.vue `applyAction` ile aynı).
/// Yalnızca bugün tüketicisi olan aksiyonlar eşlenir: confirm → Enter,
/// back → çıkış (shell kök ekrandır; ES'te kökte B = çıkış). Nav/page tuşları
/// TASK-012h'taki grid navigasyonu gelene kadar None — tüketici olmadan bağlanmaz.
pub fn gamepad_event_to_key(data: &serde_json::Value) -> Option<GamepadIntent> {
    match data.get("action").and_then(|v| v.as_str()) {
        Some("confirm") => Some(GamepadIntent::Key(UiKey::Confirm)),
        Some("back") => Some(GamepadIntent::Exit),
        _ => None,
    }
}

/// TASK-012m — `manager_update` olayını (ya da snapshot'taki `manager_update`'i) işler:
/// güncelleme mevcutsa `update_available`'a versiyonu yazar, akış aşamasını
/// `update_stage`'e yazar (TVUI prompt + bar). Hem stream event'i (`available`/`stage`
/// kökte) hem snapshot (`data["manager_update"]` nested) şeklini çözer.
fn apply_manager_update(state: &SharedTvuiState, data: &serde_json::Value) {
    // Stream event: available/stage kökte. Snapshot: data["manager_update"] nested.
    let obj = if data.get("available").is_some() || data.get("stage").is_some() {
        data
    } else if let Some(m) = data.get("manager_update") {
        m
    } else {
        return;
    };
    if obj.get("available").and_then(|v| v.as_bool()).unwrap_or(false) {
        if let Some(v) = obj.get("version").and_then(|x| x.as_str()) {
            tvui_lock(state).update_available = Some(v.to_string());
        }
    } else {
        // Faz B (bulgu 8): manager güncellemeyi geri çektiyse bayat banner
        // temizlenir; yalnızca aktif akış aşaması yoksa (in-flight korunur).
        let mut s = tvui_lock(state);
        let in_flight = matches!(
            s.update_stage.as_deref(),
            Some("downloading") | Some("ready") | Some("applying")
        );
        if !in_flight {
            s.update_available = None;
        }
    }
    if let Some(stage) = obj.get("stage").and_then(|v| v.as_str()) {
        let mut s = tvui_lock(state);
        s.update_stage = Some(stage.to_string());
        if stage == "downloading" {
            s.update_pct = obj
                .get("percent")
                .and_then(|p| p.as_u64())
                .unwrap_or(0) as u32;
        }
    }
}

/// Kullanıcı-tetiklemeli API çağrısı sonucu: makine-okunur `ok` + insan-okunur
/// `message` (log/i18n kaynağı). State geçişleri YALNIZCA `ok` üzerinden alınır —
/// human-message string-eşlemesi yasak (TASK-012-gap-01 bulgu 5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriggerResult {
    pub ok: bool,
    pub message: String,
}

impl TriggerResult {
    fn new(ok: bool, message: impl Into<String>) -> Self {
        Self {
            ok,
            message: message.into(),
        }
    }
}

/// TASK-012m Faz 5 — kullanıcı `Enter` ile indirmeyi arka plana (kuyruğa) yollar.
/// Non-blocking: hemen `{ok, queued}` döner; ilerleme SSE ile gelir.
pub fn trigger_update_download(port: u16) -> TriggerResult {
    let url = format!("http://127.0.0.1:{port}/api/manager-update/download");
    match api_agent().post(&url).call() {
        Ok(r) => match r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => parse_download_response(&v),
            None => TriggerResult::new(false, "yanıt çözülemedi"),
        },
        Err(e) => TriggerResult::new(false, format!("istek hatası: {e}")),
    }
}

/// TASK-012m Faz 5 — `Enter` ile indirilmiş güncellemeyi uygular (replace + relaunch).
/// GERİ ALINAMAZ; sunucu yalnız `RGSX_SELF_APPLY=1` ile gerçekleştirir.
pub fn trigger_update_apply(port: u16) -> TriggerResult {
    let url = format!("http://127.0.0.1:{port}/api/manager-update/apply");
    match api_agent().post(&url).call() {
        Ok(r) => match r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => parse_apply_response(&v),
            None => TriggerResult::new(false, "yanıt çözülemedi"),
        },
        Err(e) => TriggerResult::new(false, format!("istek hatası: {e}")),
    }
}

/// TASK-012m Faz 5 — yanlış tık: self-update indirmesini kuyruktan iptal eder
/// (WebUI/Python TVUI parity). `task_id = "manager-update"`.
pub fn trigger_update_cancel(port: u16) -> TriggerResult {
    let url = format!("http://127.0.0.1:{port}/api/queue/remove");
    let body = serde_json::json!({ "task_id": MANAGER_UPDATE_TASK_ID });
    match api_agent()
        .post(&url)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
    {
        Ok(_) => TriggerResult::new(true, "indirme iptal edildi"),
        Err(e) => TriggerResult::new(false, format!("iptal hatası: {e}")),
    }
}

/// `manager-update/download` yanıtını çözer (`ok` makine-alanı + placeholder mesaj).
fn parse_download_response(v: &serde_json::Value) -> TriggerResult {
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        if v.get("queued").and_then(|x| x.as_bool()).unwrap_or(false) {
            TriggerResult::new(true, "indirme kuyruğa alındı")
        } else {
            TriggerResult::new(
                true,
                format!(
                    "indirildi: {}",
                    v.get("path").and_then(|x| x.as_str()).unwrap_or("")
                ),
            )
        }
    } else {
        TriggerResult::new(
            false,
            format!(
                "hata: {}",
                v.get("error").and_then(|x| x.as_str()).unwrap_or("bilinmiyor")
            ),
        )
    }
}

/// `manager-update/apply` yanıtını çözer (`ok` makine-alanı + placeholder mesaj).
fn parse_apply_response(v: &serde_json::Value) -> TriggerResult {
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        TriggerResult::new(true, "yeniden başlatılıyor")
    } else {
        TriggerResult::new(
            false,
            format!(
                "hata: {}",
                v.get("error").and_then(|x| x.as_str()).unwrap_or("bilinmiyor")
            ),
        )
    }
}

/// TASK — bootstrap fail sonrası katalog hazırlanmasını yeniden dener
/// (manager-http `/api/catalog/retry`). Sunucu arka planda bootstrap'i tekrar
/// çalıştırır; ilerleme SSE `catalog_update` ile gelir.
pub fn trigger_catalog_retry(port: u16) -> TriggerResult {
    let url = format!("http://127.0.0.1:{port}/api/catalog/retry");
    match api_agent().post(&url).call() {
        Ok(r) => match r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => parse_retry_response(&v),
            None => TriggerResult::new(false, "yanıt çözülemedi"),
        },
        Err(e) => TriggerResult::new(false, format!("istek hatası: {e}")),
    }
}

/// `/api/catalog/retry` yanıtını çözer (`success` makine-alanı + placeholder mesaj).
fn parse_retry_response(v: &serde_json::Value) -> TriggerResult {
    if v.get("success").and_then(|x| x.as_bool()).unwrap_or(false) {
        TriggerResult::new(true, "yeniden deneniyor")
    } else {
        TriggerResult::new(
            false,
            format!(
                "hata: {}",
                v.get("error").and_then(|x| x.as_str()).unwrap_or("bilinmiyor")
            ),
        )
    }
}

/// `port` üzerindeki manager-http'e SSE bağlanır; `catalog_update`, `snapshot`,
/// `manager_update` ve `gamepad` olaylarını işler.
///
/// Dayanıklılık (TASK-012-gap-01 Faz A): bağlantı kurulamazsa YA DA akış koparsa
/// (EOF / okuma stall'ı > SSE_READ_TIMEOUT) `SSE_RECONNECT_DELAY` bekleyip sonsuza dek
/// yeniden dener (Python parity: tvui.py `_manager_sse_worker`). Yalnızca İLK bağlantı
/// hatası `state.error`'a yazılır; sonraki kopmalar mevcut UI durumunu bozmaz —
/// self-update apply gibi manager restart'larında TVUI kendini toparlar.
///
/// Faz C (bulgu 9): `shutdown` set edilince (gamepad back / shell çıkışı)
/// reconnect döngüsü temizce biter — sızan thread kalmaz.
pub fn start_catalog_watcher(port: u16, state: SharedTvuiState, shutdown: &AtomicBool) {
    tvui_lock(&state).port = port;
    let url = format!("http://127.0.0.1:{port}/api/events");
    // Connect'e kısa timeout; akış uzun ömürlü olduğu için toplam timeout YOK,
    // yalnızca read watchdog'u var (stall'da read Err döner → reconnect).
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(SSE_READ_TIMEOUT)
        .build();
    let mut ever_connected = false;
    loop {
        // Faz C: shell kapattıysa (gamepad back) watcher da sönmez, temiz biter.
        if shutdown.load(Ordering::Relaxed) {
            return;
        }
        match agent.get(&url).call() {
            Ok(resp) => {
                eprintln!("TVUI SSE bağlı: {url}");
                ever_connected = true;
                consume_sse_stream(resp, port, &state, shutdown);
                eprintln!(
                    "TVUI SSE akışı kapandı, {SSE_RECONNECT_DELAY:?} sonra yeniden bağlanıyor"
                );
            }
            Err(e) => {
                if !ever_connected {
                    tvui_lock(&state).error = Some(format!("SSE baglanti hatasi: {e}"));
                }
                eprintln!("TVUI SSE baglanti hatasi ({e}), yeniden denenecek");
            }
        }
        std::thread::sleep(SSE_RECONNECT_DELAY);
    }
}

/// Açık SSE yanıtını satır satır tüketir; tamamlanan frame'leri `state`'e uygular.
/// Bağlantı EOF/hata/stall ile biter (fonksiyon geri döner) — reconnect döngüsü üstlenir.
fn consume_sse_stream(
    resp: ureq::Response,
    port: u16,
    state: &SharedTvuiState,
    shutdown: &AtomicBool,
) {
    let reader = std::io::BufReader::new(resp.into_reader());
    let mut acc = String::new();
    for line in reader.lines() {
        let Ok(line) = line else {
            break; // io hatası / read-timeout stall'ı → reconnect
        };
        if line.is_empty() {
            handle_sse_frame(&acc, port, state, shutdown);
            acc.clear();
        } else {
            acc.push_str(&line);
            acc.push('\n');
        }
    }
}

/// Tek tamamlanmış SSE frame'ini olay türüne göre uygular; `ready` olup platformlar
/// hâlâ boşsa `/api/platforms`'ı çeker (grid kaynağı).
fn handle_sse_frame(acc: &str, port: u16, state: &SharedTvuiState, shutdown: &AtomicBool) {
    if let Some((ev, data)) = parse_sse_frame(acc) {
        match ev.as_str() {
            "catalog_update" => apply_catalog_update(state, &data),
            "snapshot" => {
                // Race düzeltmesi: geç bağlanan TVUI, başlangıç snapshot'ından katalogun
                // hazır olduğunu görür ve loading bar'ını kapatır (catalog_update kaçırılsa da).
                apply_snapshot(state, &data);
                apply_manager_update(state, &data);
            }
            "manager_update" => apply_manager_update(state, &data),
            // Faz C (bulgu 9): gilrs tabanlı native gamepad SSE üzerinden shell'e gelir.
            "gamepad" => match gamepad_event_to_key(&data) {
                Some(GamepadIntent::Exit) => shutdown.store(true, Ordering::Relaxed),
                Some(GamepadIntent::Key(key)) => {
                    let action = {
                        let s = tvui_lock(state);
                        ui_decision(&s, key)
                    };
                    if let Some(action) = action {
                        apply_ui_action(state, action);
                    }
                }
                None => {}
            },
            _ => {}
        }
    }
    let (ready, empty) = {
        let s = tvui_lock(state);
        (s.ready, s.platforms.is_empty())
    };
    if ready && empty {
        tvui_lock(state).platforms = fetch_platforms(port);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sse_frame_extracts_event_and_data() {
        let frame = "event: catalog_update\ndata: {\"stage\":\"download\",\"pct\":42,\"total\":1000}\n\n";
        let (ev, data) = parse_sse_frame(frame).expect("frame cozulmeli");
        assert_eq!(ev, "catalog_update");
        assert_eq!(data["pct"], 42);
        assert_eq!(data["stage"], "download");
    }

    #[test]
    fn parse_sse_frame_returns_none_without_event_or_data() {
        assert!(parse_sse_frame("data: {\"a\":1}\n\n").is_none());
        assert!(parse_sse_frame("event: x\n\n").is_none());
        assert!(parse_sse_frame("").is_none());
    }

    #[test]
    fn apply_catalog_update_sets_pct_and_ready() {
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_catalog_update(
            &state,
            &serde_json::json!({"stage":"download","pct":10,"total":500}),
        );
        assert_eq!(state.lock().unwrap().pct, 10);
        apply_catalog_update(
            &state,
            &serde_json::json!({"stage":"ready","success":true}),
        );
        let s = state.lock().unwrap();
        assert!(s.ready);
        assert!(!s.loading);
        assert_eq!(s.pct, 100);
    }

    #[test]
    fn apply_catalog_update_failure_keeps_not_ready() {
        // Bootstrap fail olunca ready=true olmamali (eski bug: bos grid'e atliyordu).
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_catalog_update(
            &state,
            &serde_json::json!({"stage":"ready","success":false,"reason":"no_source"}),
        );
        let s = state.lock().unwrap();
        assert!(!s.ready, "fail olunca ready=true olmamali");
        assert!(!s.loading);
        assert!(s.error.is_some());
        assert!(s.error.as_ref().unwrap().contains("no_source"));
    }

    #[test]
    fn parse_retry_response_reads_success() {
        let ok_v = serde_json::json!({"success": true, "retrying": true});
        let ok = parse_retry_response(&ok_v);
        assert!(ok.ok);
        assert!(ok.message.contains("yeniden"));
        let err_v = serde_json::json!({"success": false, "error": "kapali"});
        let err = parse_retry_response(&err_v);
        assert!(!err.ok);
        assert!(err.message.contains("kapali"));
    }

    #[test]
    fn snapshot_catalog_ready_marks_ready_and_clears_stale_error_offline() {
        // Race düzeltmesi: geç SSE abonesi, başlangıç snapshot'ından katalogun hazır
        // olduğunu görüp loading bar'ını kapatmalı (catalog_update kaçırılsa bile).
        // Faz A: hazır katalogda eski hata/çevrimdışı bayrakları bayatmıştır → temizlenir.
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        {
            let mut s = state.lock().unwrap();
            s.error = Some("katalog hazirlanamadi: no_source".to_string());
            s.offline = true;
        }
        apply_snapshot(
            &state,
            &serde_json::json!({"catalog_ready": true, "network_down": false}),
        );
        let s = state.lock().unwrap();
        assert!(s.ready);
        assert!(!s.loading);
        assert_eq!(s.pct, 100);
        assert!(s.error.is_none(), "bayat hata temizlenmeli");
        assert!(!s.offline);
    }

    #[test]
    fn parse_sse_frame_joins_multiline_data() {
        // Bulgu 14: çok satırlı data \n ile birleşir (SSE spec + Python parity).
        let frame = "event: snapshot\ndata: {\"a\":\ndata: 1}\n\n";
        let (ev, data) = parse_sse_frame(frame).expect("çok satırlı frame çözülmeli");
        assert_eq!(ev, "snapshot");
        assert_eq!(data["a"], 1);
    }

    #[test]
    fn gamepad_events_map_to_intents() {
        // Bulgu 9: confirm → Enter eşdeğeri, back → çıkış; nav/bilinmeyen → None
        // (tüketici olmadan bağlanmaz — TASK-012h'ta genişler).
        assert_eq!(
            gamepad_event_to_key(&serde_json::json!({"action": "confirm"})),
            Some(GamepadIntent::Key(UiKey::Confirm))
        );
        assert_eq!(
            gamepad_event_to_key(&serde_json::json!({"action": "back"})),
            Some(GamepadIntent::Exit)
        );
        assert_eq!(
            gamepad_event_to_key(&serde_json::json!({"action": "navup"})),
            None
        );
        assert_eq!(gamepad_event_to_key(&serde_json::json!({})), None);
    }

    #[test]
    fn gamepad_confirm_frame_drives_ui_and_back_exits() {
        // Uçtan uca (ağsız): SSE `gamepad` frame'i hata ekranında offline-devam
        // uygular; `back` shutdown bayrağını çeker.
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        state.lock().unwrap().error = Some("katalog hazirlanamadi".to_string());
        let shutdown = AtomicBool::new(false);
        handle_sse_frame(
            "event: gamepad\ndata: {\"action\":\"confirm\"}\n\n",
            59999,
            &state,
            &shutdown,
        );
        assert!(state.lock().unwrap().offline);
        assert!(!shutdown.load(Ordering::Relaxed));
        handle_sse_frame(
            "event: gamepad\ndata: {\"action\":\"back\"}\n\n",
            59999,
            &state,
            &shutdown,
        );
        assert!(shutdown.load(Ordering::Relaxed));
    }

    #[test]
    fn watcher_retries_after_failed_connect_instead_of_returning() {
        // TASK-012-gap-01 Faz A: ESKİ davranış ilk connect hatasında fonksiyondan
        // dönüyordu (thread ölür → loading bar sonsuza dek donar). YENİ davranış:
        // ilk hatayı error'a yazıp retry döngüsünde YAŞAMAYA devam eder.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral port");
        let port = listener.local_addr().unwrap().port();
        drop(listener); // Portu serbest bırak → bağlantı hızlıca REDDEDİLİR.
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        let handle = {
            let st = state.clone();
            let flag = Arc::new(AtomicBool::new(false));
            std::thread::spawn(move || start_catalog_watcher(port, st, &flag))
        };
        std::thread::sleep(Duration::from_millis(500));
        let err = state.lock().unwrap().error.clone();
        assert!(
            err.as_deref().unwrap_or_default().starts_with("SSE"),
            "ilk bağlantı hatası error'a yazılmalı, geldi: {err:?}"
        );
        assert!(
            !handle.is_finished(),
            "watcher ilk hatada ÖLMEMELİ — reconnect döngüsünde olmalı"
        );
        // Watcher'ı bilinçli olarak leak ediyoruz: sonsuz retry döngüsüdür; test
        // process'i çıkınca sonlanır (Rust'ta main exit tüm thread'leri keser).
    }

    #[test]
    fn parse_platforms_reads_name_and_folder() {
        let v = serde_json::json!({
            "count": 2,
            "platforms": [
                {"platform_name": "NES", "folder": "nes", "games_count": 10},
                {"platform_name": "Game Boy", "dossier": "gb"}
            ]
        });
        let tiles = parse_platforms(&v);
        assert_eq!(tiles.len(), 2);
        assert_eq!(tiles[0].name, "NES");
        assert_eq!(tiles[0].folder, "nes");
        // `dossier` → `folder` fallback.
        assert_eq!(tiles[1].name, "Game Boy");
        assert_eq!(tiles[1].folder, "gb");
    }

    #[test]
    fn parse_platforms_empty_when_no_array() {
        assert!(parse_platforms(&serde_json::json!({"count": 0})).is_empty());
        assert!(parse_platforms(&serde_json::json!(null)).is_empty());
    }

    #[test]
    fn apply_manager_update_sets_version_when_available() {
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_manager_update(
            &state,
            &serde_json::json!({"available": true, "version": "2.0.0", "url": "x", "sha256": "y"}),
        );
        assert_eq!(state.lock().unwrap().update_available.as_deref(), Some("2.0.0"));
        // Faz B (bulgu 8): available:false → bayat banner temizlenir.
        apply_manager_update(&state, &serde_json::json!({"available": false, "version": "9.9.9"}));
        assert_eq!(state.lock().unwrap().update_available, None);
    }

    #[test]
    fn available_false_keeps_inflight_update_flow() {
        // Bulgu 8 sınırı: aktif indirme/apply aşaması varken available:false
        // dokunmaz (in-flight akış korunur).
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        {
            let mut s = state.lock().unwrap();
            s.update_available = Some("9.9.9".to_string());
            s.update_stage = Some("downloading".to_string());
        }
        apply_manager_update(&state, &serde_json::json!({"available": false}));
        let s = state.lock().unwrap();
        assert_eq!(s.update_available.as_deref(), Some("9.9.9"));
        assert_eq!(s.update_stage.as_deref(), Some("downloading"));
    }

    #[test]
    fn apply_manager_update_handles_snapshot_nesting() {
        // Snapshot şekli: manager_update nested obje (kök `available` YOK).
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_manager_update(
            &state,
            &serde_json::json!({
                "catalog_ready": true,
                "manager_update": {"available": true, "version": "3.1.0", "url": "u", "sha256": "s"}
            }),
        );
        assert_eq!(state.lock().unwrap().update_available.as_deref(), Some("3.1.0"));
        // Stream event şekli: available kökte.
        let s2: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_manager_update(&s2, &serde_json::json!({"available": true, "version": "4.0.0"}));
        assert_eq!(s2.lock().unwrap().update_available.as_deref(), Some("4.0.0"));
    }

    #[test]
    fn parse_download_response_reads_ok_and_queued() {
        // Faz 5: download non-blocking → {ok:true, queued:true}.
        let ok_v = serde_json::json!({"success": true, "ok": true, "queued": true});
        let ok = parse_download_response(&ok_v);
        assert!(ok.ok);
        assert!(ok.message.contains("kuyruğa"));
        let err_v = serde_json::json!({"success": true, "ok": false, "error": "SHA256 uyumsuz"});
        let err = parse_download_response(&err_v);
        assert!(!err.ok);
        assert!(err.message.contains("SHA256 uyumsuz"));
    }

    #[test]
    fn apply_manager_update_parses_stage() {
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_manager_update(
            &state,
            &serde_json::json!({"available": true, "version": "5.0.0", "stage": "ready"}),
        );
        let s = state.lock().unwrap();
        assert_eq!(s.update_available.as_deref(), Some("5.0.0"));
        assert_eq!(s.update_stage.as_deref(), Some("ready"));
        assert!(!s.update_restarting);
    }

    #[test]
    fn ui_decision_covers_state_machine() {
        // Bulgu 15: tüm geçiş kuralları SDL'siz tek tabloda test edilir.
        let base = TvuiState::default();
        // Boş durumda hiçbir tuş bir şey yapmaz.
        assert_eq!(ui_decision(&base, UiKey::Confirm), None);
        assert_eq!(ui_decision(&base, UiKey::Retry), None);
        assert_eq!(ui_decision(&base, UiKey::CancelUpdate), None);
        // Hata ekranında: R → retry, Enter → çevrimdışı devam.
        let mut e = base.clone();
        e.error = Some("katalog hazirlanamadi".to_string());
        assert_eq!(ui_decision(&e, UiKey::Retry), Some(UiAction::RetryCatalog));
        assert_eq!(ui_decision(&e, UiKey::Confirm), Some(UiAction::ContinueOffline));
        // Çevrimdışıya geçildikten sonra tekrar Enter nötrdür.
        let mut off = e.clone();
        off.offline = true;
        assert_eq!(ui_decision(&off, UiKey::Confirm), None);
        // Güncelleme akışı: available → download, ready → apply, restarting → nötr.
        let mut u = base.clone();
        u.update_available = Some("2.0.0".to_string());
        assert_eq!(ui_decision(&u, UiKey::Confirm), Some(UiAction::UpdateDownload));
        u.update_stage = Some("ready".to_string());
        assert_eq!(ui_decision(&u, UiKey::Confirm), Some(UiAction::UpdateApply));
        u.update_restarting = true;
        assert_eq!(ui_decision(&u, UiKey::Confirm), None);
        // İptal yalnız downloading aşamasında anlamlı.
        let mut c = base.clone();
        c.update_stage = Some("downloading".to_string());
        assert_eq!(ui_decision(&c, UiKey::CancelUpdate), Some(UiAction::UpdateCancel));
    }

    #[test]
    fn restart_overlay_expires_after_timeout() {
        // Bulgu 7: relaunch gelmezse overlay kapanır, banner failed'e döner.
        let now = Instant::now();
        let mut s = TvuiState::default();
        s.update_restarting = true;
        s.update_restarting_since = Some(now - Duration::from_secs(61));
        assert!(expire_stale_restart_at(&mut s, now));
        assert!(!s.update_restarting);
        assert!(s.update_restarting_since.is_none());
        assert_eq!(s.update_stage.as_deref(), Some("failed"));

        // Henüz süre dolmadıysa dokunulmaz.
        let mut fresh = TvuiState::default();
        fresh.update_restarting = true;
        fresh.update_restarting_since = Some(now - Duration::from_secs(10));
        assert!(!expire_stale_restart_at(&mut fresh, now));
        assert!(fresh.update_restarting);

        // since kayıpsa güvenli tarafta kal: overlay kapatılır.
        let mut orphan = TvuiState::default();
        orphan.update_restarting = true;
        assert!(expire_stale_restart_at(&mut orphan, now));
    }

    #[test]
    fn apply_ui_action_runs_http_off_thread_and_marks_failed_on_dead_port() {
        // Bulgu 3+6: HTTP arka plan thread'inde; başarısız istek stage'i
        // downloading YAPMAZ — failed'e çeker (eski davranış banner'ı
        // sonsuza dek downloading'de bırakıyordu).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("ephemeral port");
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState {
            update_available: Some("1.0.1".to_string()),
            ..TvuiState::default()
        }));
        state.lock().unwrap().port = port;
        apply_ui_action(&state, UiAction::UpdateDownload);
        for _ in 0..100 {
            if state.lock().unwrap().update_stage.as_deref() == Some("failed") {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!("arka plan isteği 2 sn içinde 'failed' yazmalı");
    }
}
