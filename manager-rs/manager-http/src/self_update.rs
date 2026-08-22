//! TASK-012m — manager self-update (Faz 1-4 güvenli iskelet).
//!
//! Faz 5 (binary replace + relaunch) BU turda YOK — ayrı, açık onaylı alt adım.
//! Akış: versiyon manifest'i çek → `CARGO_PKG_VERSION` ile karşılaştır →
//! yeniyse SSE `manager_update` yayınla + `StateData`'ya yaz. TVUI prompt →
//! (kullanıcı onaylayınca) indir + SHA256 doğrula (üzerine YAZMA).

use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast::Sender;
use tracing::{info, warn};

use crate::state::{AppState, StateData};

use futures_util::StreamExt;
use sha2::{Digest, Sha256};
use serde_json::Value;
use std::io::Write;

/// Self-update indirmesinin kuyruk görevi kimliği (WebUI/Python TVUI parity:
/// yanlış tık → `/api/queue/remove` ile iptal). Sabit kimlik → tekrar indirme engeli.
pub const MANAGER_UPDATE_TASK_ID: &str = "manager-update";

/// Uzak versiyon manifest'i (`RGSX_UPDATE_MANIFEST_URL` ile sunulur).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub version: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
}

/// `"x.y.z"` → `(major, minor, patch)`. Parse edilemezse `None`.
pub fn parse_version(s: &str) -> Option<(u32, u32, u32)> {
    let mut it = s.trim_start_matches('v').split('.');
    let major = it.next()?.parse().ok()?;
    let minor = it.next().unwrap_or("0").parse().ok()?;
    let patch = it
        .next()
        .unwrap_or("0")
        .split('-')
        .next()
        .unwrap_or("0")
        .parse()
        .ok()?;
    Some((major, minor, patch))
}

/// `remote` > `current` mı? (basit semver karşılaştırması).
pub fn is_newer(remote: &str, current: &str) -> bool {
    match (parse_version(remote), parse_version(current)) {
        (Some(r), Some(c)) => r > c,
        _ => false,
    }
}

/// Manifest JSON'ı çeker (ağ). Yoksa `None`.
pub async fn fetch_manifest(url: &str) -> Option<VersionInfo> {
    let resp = reqwest::Client::new().get(url).send().await.ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.json::<VersionInfo>().await.ok()
}

/// Arka plan kontrolü: manifest çek → karşılaştır → yeniyse SSE + StateData.
pub async fn check_update(events: Sender<String>, state_data: Arc<RwLock<StateData>>) {
    let manifest_url = match std::env::var("RGSX_UPDATE_MANIFEST_URL") {
        Ok(u) if !u.is_empty() => u,
        _ => return, // Yapılandırılmamışsa parity gereği no-op.
    };
    let current = env!("CARGO_PKG_VERSION");
    let Some(info) = fetch_manifest(&manifest_url).await else {
        warn!("manager self-update: manifest çekilemedi ({manifest_url})");
        return;
    };
    if is_newer(&info.version, current) {
        info!("manager güncellemesi mevcut: {current} → {}", info.version);
        let payload = serde_json::json!({
            "available": true,
            "version": info.version,
            "url": info.url,
            "sha256": info.sha256,
            "stage": "available",
        });
        {
            let mut g = state_data.write().unwrap();
            g.manager_update = Some(payload.clone());
        }
        crate::sse::publish(&events, "manager_update", &payload);
    }
}

/// İndir + SHA256 doğrula (üzerine YAZMA). Başarılıysa indirilen dosya yolu.
/// `expected_sha256` verilmişse uyum zorunlu; uymazsa `Err`.
pub async fn download_and_verify(
    url: &str,
    expected_sha256: Option<&str>,
) -> Result<PathBuf, String> {
    let resp = reqwest::Client::new()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("indirme hatası: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("indirme HTTP {}", resp.status()));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| format!("gövde okuma: {e}"))?;
    // SHA256
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let actual = hex_encode(&hasher.finalize());
    if let Some(exp) = expected_sha256 {
        if actual.to_lowercase() != exp.to_lowercase() {
            return Err(format!("SHA256 uyumsuz: beklenen {exp}, gerçek {actual}"));
        }
    }
    // Geçici dosyaya yaz (üzerine yazma YOK — faz 5 ayrı).
    let tmp = std::env::temp_dir().join(format!("rgsx-manager-update-{actual}.bin"));
    std::fs::write(&tmp, &bytes).map_err(|e| format!("dosya yazma: {e}"))?;
    Ok(tmp)
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

// ---------------------------------------------------------------------------
// TASK-012m Faz 5 — arka plan indirme görevi (kuyruk + iptal edilebilir)
// ---------------------------------------------------------------------------

/// `manager_update` payload'ını günceller + SSE yayınlar (kilit sonrası).
fn set_manager_stage(state: &AppState, events: &Sender<String>, stage: &str, error: Option<&str>) {
    {
        let mut d = state.write();
        if let Some(m) = d.manager_update.as_mut() {
            m["stage"] = serde_json::json!(stage);
            if let Some(e) = error {
                m["error"] = serde_json::json!(e);
            } else if let Some(obj) = m.as_object_mut() {
                obj.remove("error");
            }
        }
        state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let payload = state.read().manager_update.clone().unwrap_or(serde_json::Value::Null);
    crate::sse::publish(events, "manager_update", &payload);
}

/// `data.queue` + `data.progress`'tan manager-update girdisini temizler.
fn cleanup_update_queue(state: &AppState) {
    let mut d = state.write();
    if let Some(pos) = d
        .queue
        .iter()
        .position(|e| e.get("task_id").and_then(Value::as_str) == Some(MANAGER_UPDATE_TASK_ID))
    {
        d.queue.remove(pos);
    }
    if let Some(obj) = d.progress.as_object_mut() {
        obj.remove(MANAGER_UPDATE_TASK_ID);
    }
    d.active = !d.queue.is_empty();
    state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Self-update indirmesini **arka plan görevi** olarak başlatır (non-blocking).
/// İndirme `data.queue`'ya girer (WebUI/Python TVUI parity → `/api/queue/remove`
/// ile iptal), `data.progress` üzerinden ilerleme yayınlanır. Tamamlanınca SHA256
/// doğrulanır; uyumluysa `manager_update["path"]` + `stage:"ready"` set edilir.
pub fn start_update_download(state: AppState, events: Sender<String>, url: String, sha256: Option<String>) {
    state.write().manager_update_cancel.store(false, std::sync::atomic::Ordering::SeqCst);
    tokio::spawn(async move {
        if let Err(e) = run_update_download(state.clone(), events.clone(), &url, sha256.as_deref()).await {
            warn!("manager self-update indirme hatası: {e}");
            set_manager_stage(&state, &events, "failed", Some(&format!("indirme hatası: {e}")));
            cleanup_update_queue(&state);
        }
    });
}

async fn run_update_download(
    state: AppState,
    events: Sender<String>,
    url: &str,
    expected_sha: Option<&str>,
) -> Result<(), String> {
    // Kuyruk girdisi + başlangıç progress (WebUI parity / iptal kaynağı).
    {
        let mut d = state.write();
        d.queue.push(serde_json::json!({
            "task_id": MANAGER_UPDATE_TASK_ID,
            "name": "manager-update",
            "platform": "manager",
            "url": url,
            "kind": "manager_update",
        }));
        d.progress = serde_json::json!({ MANAGER_UPDATE_TASK_ID: { "received": 0, "total": 0, "percent": 0 } });
        d.active = true;
        state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    set_manager_stage(&state, &events, "downloading", None);

    let client = reqwest::Client::new();
    let resp = client.get(url).send().await.map_err(|e| format!("istek: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0);
    let mut stream = resp.bytes_stream();

    let tmp = std::env::temp_dir().join("rgsx-manager-update.partial");
    let _ = std::fs::remove_file(&tmp);
    let mut file = std::fs::File::create(&tmp).map_err(|e| format!("temp oluşturma: {e}"))?;
    let mut hasher = Sha256::new();
    let mut received: u64 = 0;

    while let Some(chunk) = stream.next().await {
        // İptal sinyali (kuyruktan iptal) → temizle, başa dön.
        if state.read().manager_update_cancel.load(std::sync::atomic::Ordering::SeqCst) {
            let _ = std::fs::remove_file(&tmp);
            cleanup_update_queue(&state);
            set_manager_stage(&state, &events, "available", None);
            return Ok(());
        }
        let bytes = chunk.map_err(|e| format!("akış: {e}"))?;
        file.write_all(&bytes).map_err(|e| format!("yazma: {e}"))?;
        hasher.update(&bytes);
        received += bytes.len() as u64;
        let percent = if total > 0 { (received * 100 / total) as u64 } else { 0 };
        {
            let mut d = state.write();
            d.progress = serde_json::json!({ MANAGER_UPDATE_TASK_ID: { "received": received, "total": total, "percent": percent } });
            if let Some(m) = d.manager_update.as_mut() {
                m["stage"] = serde_json::json!("downloading");
                m["received"] = serde_json::json!(received);
                m["total"] = serde_json::json!(total);
            }
            state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let prog = state.read().progress.clone();
        crate::sse::publish(&events, "progress", &serde_json::json!({ "progress": prog, "active": true }));
    }

    // SHA256 doğrula.
    let actual = hex_encode(&hasher.finalize());
    if let Some(exp) = expected_sha {
        if actual.to_lowercase() != exp.to_lowercase() {
            let _ = std::fs::remove_file(&tmp);
            cleanup_update_queue(&state);
            set_manager_stage(&state, &events, "failed", Some(&format!("SHA256 uyumsuz (beklenen {exp}, gerçek {actual})")));
            return Err("sha256 uyuşmazlığı".into());
        }
    }
    // Geçici dosyayı SHA'lı son ada sabitle (üzerine yazma yapılmaz; apply fazında).
    let final_path = std::env::temp_dir().join(format!("rgsx-manager-update-{actual}.bin"));
    let _ = std::fs::remove_file(&final_path);
    std::fs::rename(&tmp, &final_path).map_err(|e| format!("yeniden adlandırma: {e}"))?;
    cleanup_update_queue(&state);

    {
        let mut d = state.write();
        if let Some(m) = d.manager_update.as_mut() {
            m["stage"] = serde_json::json!("ready");
            m["path"] = serde_json::json!(final_path.display().to_string());
            m["sha256_actual"] = serde_json::json!(actual);
        }
        state.dirty.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    let payload = state.read().manager_update.clone().unwrap_or(serde_json::Value::Null);
    crate::sse::publish(&events, "manager_update", &payload);
    Ok(())
}

/// Self-update indirmesini iptal eder (WebUI/Python TVUI: kuyruktan iptal).
pub fn cancel_update_download(state: &AppState) {
    state.write().manager_update_cancel.store(true, std::sync::atomic::Ordering::SeqCst);
}

// ---------------------------------------------------------------------------
// TASK-012m Faz 5 — apply (replace + relaunch). GERİ ALINAMAZ; yalnız
// `RGSX_SELF_APPLY=1` ile çalışır (kullanıcının açık "evet"i = flag).
// ---------------------------------------------------------------------------

/// İndirilen binary'yi çalışan exe'nin yerine koy + relaunch.
/// Serviste (`RGSX_SERVICE=1`) reddedilir. Gerçek replace yalnız `RGSX_SELF_APPLY=1`
/// ile; aksi halde güvenli şekilde hata döner (henüz uygulanmadı).
pub async fn apply_update(state: AppState, events: Sender<String>) -> Result<(), String> {
    if std::env::var("RGSX_SERVICE").map(|v| v == "1").unwrap_or(false) {
        warn!("manager self-update: servis ortamında apply reddedildi");
        return Err("servis ortamında uygulama devre dışı (RGSX_SERVICE=1)".into());
    }
    if std::env::var("RGSX_SELF_APPLY").map(|v| v == "1").unwrap_or(false) == false {
        return Err("self-apply devre dışı — RGSX_SELF_APPLY=1 gerekli".into());
    }
    // SHA + versiyon kapıları (apply'da da zorunlu).
    let (path, expected_sha) = {
        let d = state.read();
        let m = d.manager_update.clone().ok_or("indirilmiş güncelleme yok (önce indir)")?;
        let p = m.get("path").and_then(|x| x.as_str()).map(|s| s.to_string()).ok_or("indirilmiş güncelleme yolu yok")?;
        let s = m.get("sha256").and_then(|x| x.as_str()).map(|s| s.to_string());
        (p, s)
    };
    if let Some(exp) = expected_sha {
        let bytes = std::fs::read(&path).map_err(|e| format!("oku: {e}"))?;
        let mut h = Sha256::new();
        h.update(&bytes);
        let actual = hex_encode(&h.finalize());
        if actual.to_lowercase() != exp.to_lowercase() {
            return Err(format!("SHA256 uyumsuz: {actual} != {exp}"));
        }
    }
    set_manager_stage(&state, &events, "applying", None);
    // .old yedeği (rollback: manager-bin --recover).
    let current = std::env::current_exe().map_err(|e| format!("current_exe: {e}"))?;
    let old = current.with_extension("old");
    let _ = std::fs::remove_file(&old);
    std::fs::copy(&current, &old).map_err(|e| format!(".old yedek: {e}"))?;
    replace_and_relaunch(&path, &current)
}

#[cfg(windows)]
fn replace_and_relaunch(src: &str, dst: &std::path::Path) -> Result<(), String> {
    // Çalışan exe kilitli → ayrı updater süreci: 1s bekle, move, start.
    let cmd = format!(
        "timeout /t 1 /nobreak >nul & move /y \"{src}\" \"{dst}\" & start \"\" \"{dst}\"",
        src = src,
        dst = dst.display()
    );
    std::process::Command::new("cmd")
        .args(["/c", &cmd])
        .spawn()
        .map_err(|e| format!("updater süreci: {e}"))?;
    std::process::exit(0);
}

#[cfg(unix)]
fn replace_and_relaunch(src: &str, dst: &std::path::Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    std::fs::rename(src, dst).map_err(|e| format!("rename: {e}"))?;
    // Aynı PID'de yeni binary'yi execve ile çalıştır.
    let args: Vec<String> = std::env::args().collect();
    let mut cmd = std::process::Command::new(dst);
    cmd.args(&args[1..]);
    let err = cmd.exec();
    Err(format!("execve: {err}"))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        assert_eq!(parse_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.2"), Some((1, 2, 0)));
        assert_eq!(parse_version("1.2.3-rc1"), Some((1, 2, 3)));
        assert_eq!(parse_version("x.y.z"), None);
    }

    #[test]
    fn is_newer_semver() {
        assert!(is_newer("1.2.4", "1.2.3"));
        assert!(is_newer("2.0.0", "1.9.9"));
        assert!(!is_newer("1.2.3", "1.2.3"));
        assert!(!is_newer("1.2.2", "1.2.3"));
    }
}
