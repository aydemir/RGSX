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
use std::time::Duration;
use tokio::sync::broadcast::{self, Sender};
use tokio::sync::broadcast::error::RecvError;

use manager_core::contract;

use crate::state::{AppState, StateData};

use futures_util::StreamExt;

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
    contract::snapshot(
        &serde_json::json!(data.history),
        &serde_json::json!(data.queue),
        data.active,
        &data.progress,
        &data.downloaded,
    )
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

    let first = futures_util::stream::once(async move { Ok::<_, axum::Error>(snapshot.into_bytes()) });

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
            (header::CONTENT_TYPE, "text/event-stream; charset=utf-8".to_string()),
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