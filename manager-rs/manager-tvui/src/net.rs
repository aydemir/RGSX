//! TASK-012h Faz 2c - TVUI tarafı SSE istemcisi (senkron, ureq).
//!
//! manager-http `/api/events` akisini dinler; `catalog_update` olaylarini
//! paylasilan `TvuiState`'e yazar. SDL2 dongusu bunu okuyup loading bar'ini cizer.
//! Senkron olmasi bilincli: SDL2 event loop tek thread, async/tokio agirligi gereksiz.

use std::io::BufRead;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Self-update indirmesinin kuyruk görevi kimliği (manager-http ile aynı sabit).
pub const MANAGER_UPDATE_TASK_ID: &str = "manager-update";

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
pub fn parse_sse_frame(buf: &str) -> Option<(String, serde_json::Value)> {
    let mut event = String::new();
    let mut data = String::new();
    for line in buf.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data:") {
            data = rest.trim().to_string();
        }
    }
    if event.is_empty() || data.is_empty() {
        return None;
    }
    serde_json::from_str(&data).ok().map(|v| (event, v))
}

fn apply_catalog_update(state: &SharedTvuiState, data: &serde_json::Value) {
    let mut s = state.lock().unwrap();
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
fn apply_snapshot(state: &SharedTvuiState, data: &serde_json::Value) {
    if let Some(true) = data.get("catalog_ready").and_then(|v| v.as_bool()) {
        let mut s = state.lock().unwrap();
        s.ready = true;
        s.loading = false;
        s.pct = 100;
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
    match ureq::get(&url).call() {
        Ok(r) => r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .map(|v| parse_platforms(&v))
            .unwrap_or_default(),
        Err(_) => Vec::new(),
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
            state.lock().unwrap().update_available = Some(v.to_string());
        }
    }
    if let Some(stage) = obj.get("stage").and_then(|v| v.as_str()) {
        let mut s = state.lock().unwrap();
        s.update_stage = Some(stage.to_string());
        if stage == "downloading" {
            s.update_pct = obj
                .get("percent")
                .and_then(|p| p.as_u64())
                .unwrap_or(0) as u32;
        }
    }
}

/// TASK-012m Faz 5 — kullanıcı `Enter` ile indirmeyi arka plana (kuyruğa) yollar.
/// Non-blocking: hemen `{ok, queued}` döner; ilerleme SSE ile gelir.
pub fn trigger_update_download(port: u16) -> String {
    let url = format!("http://127.0.0.1:{port}/api/manager-update/download");
    match ureq::post(&url).call() {
        Ok(r) => match r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => parse_download_response(&v),
            None => "yanıt çözülemedi".to_string(),
        },
        Err(e) => format!("istek hatası: {e}"),
    }
}

/// TASK-012m Faz 5 — `Enter` ile indirilmiş güncellemeyi uygular (replace + relaunch).
/// GERİ ALINAMAZ; sunucu yalnız `RGSX_SELF_APPLY=1` ile gerçekleştirir.
pub fn trigger_update_apply(port: u16) -> String {
    let url = format!("http://127.0.0.1:{port}/api/manager-update/apply");
    match ureq::post(&url).call() {
        Ok(r) => match r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => parse_apply_response(&v),
            None => "yanıt çözülemedi".to_string(),
        },
        Err(e) => format!("istek hatası: {e}"),
    }
}

/// TASK-012m Faz 5 — yanlış tık: self-update indirmesini kuyruktan iptal eder
/// (WebUI/Python TVUI parity). `task_id = "manager-update"`.
pub fn trigger_update_cancel(port: u16) -> String {
    let url = format!("http://127.0.0.1:{port}/api/queue/remove");
    let body = serde_json::json!({ "task_id": MANAGER_UPDATE_TASK_ID });
    match ureq::post(&url)
        .set("Content-Type", "application/json")
        .send_string(&body.to_string())
    {
        Ok(_) => "indirme iptal edildi".to_string(),
        Err(e) => format!("iptal hatası: {e}"),
    }
}

/// `manager-update/download` yanıtını placeholder mesaja çözer.
fn parse_download_response(v: &serde_json::Value) -> String {
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        if v.get("queued").and_then(|x| x.as_bool()).unwrap_or(false) {
            "indirme kuyruğa alındı".to_string()
        } else {
            format!("indirildi: {}", v.get("path").and_then(|x| x.as_str()).unwrap_or(""))
        }
    } else {
        format!("hata: {}", v.get("error").and_then(|x| x.as_str()).unwrap_or("bilinmiyor"))
    }
}

/// `manager-update/apply` yanıtını placeholder mesaja çözer.
fn parse_apply_response(v: &serde_json::Value) -> String {
    if v.get("ok").and_then(|x| x.as_bool()).unwrap_or(false) {
        "yeniden başlatılıyor".to_string()
    } else {
        format!("hata: {}", v.get("error").and_then(|x| x.as_str()).unwrap_or("bilinmiyor"))
    }
}

/// TASK — bootstrap fail sonrası katalog hazırlanmasını yeniden dener
/// (manager-http `/api/catalog/retry`). Sunucu arka planda bootstrap'i tekrar
/// çalıştırır; ilerleme SSE `catalog_update` ile gelir. Sonuç mesajı döner.
pub fn trigger_catalog_retry(port: u16) -> String {
    let url = format!("http://127.0.0.1:{port}/api/catalog/retry");
    match ureq::post(&url).call() {
        Ok(r) => match r
            .into_string()
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        {
            Some(v) => parse_retry_response(&v),
            None => "yanıt çözülemedi".to_string(),
        },
        Err(e) => format!("istek hatası: {e}"),
    }
}

/// `/api/catalog/retry` yanıtını placeholder mesaja çözer.
fn parse_retry_response(v: &serde_json::Value) -> String {
    if v.get("success").and_then(|x| x.as_bool()).unwrap_or(false) {
        "yeniden deneniyor".to_string()
    } else {
        format!("hata: {}", v.get("error").and_then(|x| x.as_str()).unwrap_or("bilinmiyor"))
    }
}

/// `port` izerindeki manager-http'e SSE baglanir, `catalog_update` olaylarini `state`'e yazar.
/// Baglanti kurulamazsa `state.error` set edip doner (UI yine render eder, bar 0'da kalir).
pub fn start_catalog_watcher(port: u16, state: SharedTvuiState) {
    state.lock().unwrap().port = port;
    let url = format!("http://127.0.0.1:{port}/api/events");
    // Yalnizca connect timeout; SSE akisi uzun omurlu oldugu icin read timeout YOK.
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .build();
    let resp = match agent.get(&url).call() {
        Ok(r) => r,
        Err(e) => {
            let mut s = state.lock().unwrap();
            s.error = Some(format!("SSE baglanti hatasi: {e}"));
            return;
        }
    };
    let reader = std::io::BufReader::new(resp.into_reader());
    let mut acc = String::new();
    for line in reader.lines().map(|l| l.unwrap_or_default()) {
        if line.is_empty() {
            if let Some((ev, data)) = parse_sse_frame(&acc) {
                if ev == "catalog_update" {
                    apply_catalog_update(&state, &data);
                } else if ev == "snapshot" {
                    // Race düzeltmesi: geç bağlanan TVUI, başlangıç snapshot'ından katalogun
                    // hazır olduğunu görür ve loading bar'ını kapatır (catalog_update kaçırılsa da).
                    apply_snapshot(&state, &data);
                    apply_manager_update(&state, &data);
                } else if ev == "manager_update" {
                    apply_manager_update(&state, &data);
                }
            }
            // Faz 2e: ready olunca bir kez platformları çek (grid kaynağı).
            let (ready, empty) = {
                let s = state.lock().unwrap();
                (s.ready, s.platforms.is_empty())
            };
            if ready && empty {
                let mut s = state.lock().unwrap();
                s.platforms = fetch_platforms(port);
            }
            acc.clear();
        } else {
            acc.push_str(&line);
            acc.push('\n');
        }
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
        assert!(parse_retry_response(&ok_v).contains("yeniden"));
        let err_v = serde_json::json!({"success": false, "error": "kapali"});
        assert!(parse_retry_response(&err_v).contains("kapali"));
    }

    #[test]
    fn snapshot_catalog_ready_marks_ready() {
        // Race düzeltmesi: geç SSE abonesi, başlangıç snapshot'ından katalogun hazır
        // olduğunu görüp loading bar'ını kapatmalı (catalog_update kaçırılsa bile).
        let state: SharedTvuiState = Arc::new(Mutex::new(TvuiState::default()));
        apply_snapshot(
            &state,
            &serde_json::json!({"catalog_ready": true, "network_down": false}),
        );
        let s = state.lock().unwrap();
        assert!(s.ready);
        assert!(!s.loading);
        assert_eq!(s.pct, 100);
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
        // available:false → ayarlanmaz.
        apply_manager_update(&state, &serde_json::json!({"available": false, "version": "9.9.9"}));
        assert_eq!(state.lock().unwrap().update_available.as_deref(), Some("2.0.0"));
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
        assert!(parse_download_response(&ok_v).contains("kuyruğa"));
        let err_v = serde_json::json!({"success": true, "ok": false, "error": "SHA256 uyumsuz"});
        assert!(parse_download_response(&err_v).contains("SHA256 uyumsuz"));
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
}
