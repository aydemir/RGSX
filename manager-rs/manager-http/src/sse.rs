//! SSE: `/api/events` event kanalı + snapshot broadcast.
//!
//! TASK-002b — Python `rgsx_manager.py` sözleşmesi 1:1:
//! - Bağlantıda anlık `snapshot` olayı (`_sse_event("snapshot", _build_snapshot())`).
//! - Sonra bireysel diff olayları: `history`, `queue`, `progress`, `downloaded`.
//! - Format: `event: <type>\ndata: <json>\n\n` (`contract::sse_event`).
//!
//! İş mantığı placeholder: durum değişikliklerini publish eden çağrılar
//! TASK-002c (bridge/bin) ile bağlanır; burada kanal artık hazır.

use axum::body::Body;
use axum::extract::State;
use axum::http::header;
use axum::response::IntoResponse;
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::{self, Sender};
use tokio::time::interval;

use manager_core::contract;

use crate::state::{AppState, QueueStatus, StateData};

use futures_util::StreamExt;
use serde_json::json;
use serde_json::Value;

/// Broadcast kanal kapasitesi — Python'un `_broadcaster_loop`'u ~250ms'de bir
/// yayın yapar; kapasite yetmese `Lagged` ile atlanır (Python kaybı gibi).
pub const CHANNEL_CAPACITY: usize = 256;

/// SSE olay kanalı oluşturur (sunucu başlangıcında bir kez).
pub fn channel() -> Sender<String> {
    broadcast::channel(CHANNEL_CAPACITY).0
}

/// `_broadcast(event_type, data)` (rgsx_manager.py:112-122) — kanala ham SSE
/// metni gönderir; abone yoksa sükunetle döner (Python davranışı).
pub fn publish(sender: &Sender<String>, event_type: &str, data: &serde_json::Value) {
    let raw = contract::sse_event(event_type, data);
    let _ = sender.send(raw);
}

/// `_build_snapshot()` (rgsx_manager.py:86-109) ile birebir snapshot yükü.
pub fn snapshot_json(data: &StateData) -> serde_json::Value {
    let mut snap = contract::snapshot(
        &serde_json::json!(data.history),
        &serde_json::json!(data.queue),
        data.active,
        &data.progress,
        &data.downloaded,
    );
    // F2/gap-30: webui'nin duraklatma durumunu görmesi için `status`'u snapshot'a ekle.
    // QueueStatus Serialize derive'ı yok → açık string'e çevir.
    let s = match data.status {
        QueueStatus::Running => "Running",
        QueueStatus::Paused => "Paused",
        QueueStatus::Stopped => "Stopped",
    };
    if let Some(obj) = snap.as_object_mut() {
        obj.insert("status".into(), serde_json::json!(s));
        // TASK-002-gap-32: UI'nin ağ-koptu durumunu görmesi (banner + "Ağ bekleniyor").
        obj.insert(
            "network_down".into(),
            serde_json::json!(data.network_down.load(Ordering::Relaxed)),
        );
        // Faz 2c-race: katalog bootstrap durumu TVUI'ye snapshot'ta sinyal (geç abone kurtulur).
        obj.insert(
            "catalog_ready".into(),
            serde_json::json!(data.catalog_ready.load(Ordering::Relaxed)),
        );
        obj.insert(
            "catalog_error".into(),
            serde_json::json!(data.catalog_error),
        );
        // TASK-012m: manager self-update durumu TVUI'ye snapshot'ta sinyal.
        obj.insert(
            "manager_update".into(),
            serde_json::json!(data.manager_update),
        );
    }
    snap
}

/// SSE endpoint handler: bağlantıda snapshot, sonra canlı olaylar.
///
/// Brüt `text/event-stream` (axum `Sse`'nin kendi `data:` satırı eklemesi yerine
/// Python `_handle_sse` ile 1:1 ham format yazılır); 15s sessizlikte snapshot
/// tekrarı Python timeout refresh'ini taklit eder (bu slice'da keep-alive yok).
pub async fn events(State(state): State<AppState>) -> impl IntoResponse {
    let rx = state.events.subscribe();

    let snapshot = {
        let data = state.read();
        contract::sse_event("snapshot", &snapshot_json(&data))
    };

    let first =
        futures_util::stream::once(async move { Ok::<_, axum::Error>(snapshot.into_bytes()) });

    let rest = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok(raw) => return Some((Ok::<_, axum::Error>(raw.into_bytes()), rx)),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => return None,
            }
        }
    });

    let stream = first.chain(rest);

    (
        [
            (
                header::CONTENT_TYPE,
                "text/event-stream; charset=utf-8".to_string(),
            ),
            (header::CACHE_CONTROL, "no-cache".to_string()),
            (header::CONNECTION, "keep-alive".to_string()),
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".to_string()),
        ],
        Body::from_stream(stream),
    )
        .into_response()
}

/// Keep-alive aralığı (Python 15s timeout → boşta snapshot yeniden gönderimi).
#[allow(dead_code)]
pub const IDLE_SNAPSHOT_INTERVAL: Duration = Duration::from_secs(15);

/// F6 — 250ms batched delta SSE broadcaster (Python `_broadcaster_loop` parity'si,
/// `rgsx_manager.py:125`).
///
/// Durum bölümleri (history / queue / progress / downloaded) okuma kilidi
/// yalnızca serileştirme süresince tutulur (mikrosaniyeler); değişen bölümler
/// `publish` ile tek seferde yayınlanır. Böylece bayt/transafer başına anlık SSE
/// seli ortadan kalkar, WebUI en fazla 250ms gecikmeyle güncellenir. 30s'de bir
/// tam `snapshot` (keep-alive + tam state refresh) yayınlanır.
pub async fn broadcast_loop(state: AppState) {
    let mut ticker = interval(Duration::from_millis(250));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut last_history: Option<String> = None;
    let mut last_queue: Option<String> = None;
    let mut last_progress: Option<String> = None;
    let mut last_downloaded: Option<String> = None;
    let mut last_snapshot = Instant::now();
    loop {
        ticker.tick().await;
        // 30s tam-snapshot: `dirty` bayrağından BAĞIMSIZ (keep-alive + tam state
        // refresh). Bir `dirty` set'i atlanırsa en fazla 30s bayatlar, kalıcı
        // SSE uyumsuzluğu oluşmaz.
        if last_snapshot.elapsed() >= Duration::from_secs(30) {
            let snap = {
                let d = state.read();
                snapshot_json(&d)
            };
            publish(&state.events, "snapshot", &snap);
            last_snapshot = Instant::now();
            state.dirty.store(false, Ordering::Relaxed);
            continue;
        }
        // Değişim yoksa serialization'ı tamamen atla (idle daemon CPU tasarrufu).
        // Yalnızca durum değiştiğinde (`dirty == true`) serileştir + yayın yapılır.
        if !state.dirty.load(Ordering::Relaxed) {
            continue;
        }
        // Okuma kilidi yalnızca serileştirme kadar tutulur (F3-F4 lock granularity).
        let (history, queue, active, progress, downloaded) = {
            let d = state.read();
            (
                serde_json::to_string(&d.history).unwrap_or_default(),
                serde_json::to_string(&d.queue).unwrap_or_default(),
                d.active,
                serde_json::to_string(&d.progress).unwrap_or_default(),
                serde_json::to_string(&d.downloaded).unwrap_or_default(),
            )
        };
        if last_history.as_deref() != Some(history.as_str()) {
            last_history = Some(history.clone());
            publish(
                &state.events,
                "history",
                &json!({ "history": parse_value(&history) }),
            );
        }
        if last_queue.as_deref() != Some(queue.as_str()) {
            last_queue = Some(queue.clone());
            publish(
                &state.events,
                "queue",
                &json!({ "queue": parse_value(&queue), "active": active }),
            );
        }
        if last_progress.as_deref() != Some(progress.as_str()) {
            last_progress = Some(progress.clone());
            publish(
                &state.events,
                "progress",
                &json!({ "progress": parse_value(&progress), "active": active }),
            );
        }
        if last_downloaded.as_deref() != Some(downloaded.as_str()) {
            last_downloaded = Some(downloaded.clone());
            publish(
                &state.events,
                "downloaded",
                &json!({ "downloaded": parse_value(&downloaded) }),
            );
        }
        state.dirty.store(false, Ordering::Relaxed);
    }
}

/// Serileştirilmiş bölümü yayın için `Value`'a çözer (değişmediyse çağrılmaz).
fn parse_value(s: &str) -> Value {
    serde_json::from_str::<Value>(s).unwrap_or(Value::Null)
}
