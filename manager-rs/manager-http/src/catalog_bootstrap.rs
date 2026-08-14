//! Faz 12f — native katalog bootstrap (saf-Rust, no Python).
//!
//! `RGSX_NATIVE_CATALOG=1` ama `RGSX_DATA_DIR` içinde `systems_list.json` ve
//! `games/*.json` yoksa, OTA `games.zip`'i indirip çıkarır. Python
//! `rgsx_cli.ensure_data_present` mantığının birebir Rust karşılığı — böylece
//! saf-Rust `manager-bin` kopyası katalog verisi olmadan "boş kategori" döndürmez.
//!
//! Zip URL çözümü (Python `get_sources_zip_url` eşleniği):
//! - `RGSX_SOURCES_MODE=custom` + `RGSX_SOURCES_ZIP_URL` (http/https) → o URL.
//! - custom modunda URL boşsa `RGSX_DATA_DIR/games.zip` yerel dosyasına düş.
//! - custom değilse `RGSX_SOURCES_ZIP_URL` veya varsayılan OTA URL'i.

use std::path::{Path, PathBuf};

use futures_util::StreamExt;
use tracing::{info, warn};

/// Varsayılan OTA kaynak ZIP'i (`config.OTA_data_ZIP` eşleniği).
const DEFAULT_SOURCES_ZIP_URL: &str = "https://retrogamesets.fr/softs/games.zip";

/// `RGSX_DATA_DIR` (yoksa `.`). `NativeCatalog::from_env` ile aynı yolu kullanır.
fn default_data_dir() -> PathBuf {
    std::env::var("RGSX_DATA_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// `systems_list.json` + en az bir `games/*.json` var mı?
fn catalog_present(data_dir: &Path) -> bool {
    if !data_dir.join("systems_list.json").is_file() {
        return false;
    }
    let games = data_dir.join("games");
    if !games.is_dir() {
        return false;
    }
    match std::fs::read_dir(&games) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .any(|e| {
                e.path()
                    .extension()
                    .map(|x| x.eq_ignore_ascii_case("json"))
                    .unwrap_or(false)
            }),
        Err(_) => false,
    }
}

/// Zip kaynağını çöz: custom/http yerel veya varsayılan OTA. `None` → kaynak yok.
fn resolve_zip_source() -> Option<String> {
    let mode = std::env::var("RGSX_SOURCES_MODE").unwrap_or_default();
    if mode.eq_ignore_ascii_case("custom") {
        if let Some(u) = std::env::var("RGSX_SOURCES_ZIP_URL")
            .ok()
            .filter(|s| !s.is_empty())
        {
            if u.starts_with("http://") || u.starts_with("https://") {
                return Some(u);
            }
        }
        // custom modunda yerel games.zip'e düş
        let local = default_data_dir().join("games.zip");
        if local.is_file() {
            return Some(local.to_string_lossy().to_string());
        }
        return None;
    }
    Some(
        std::env::var("RGSX_SOURCES_ZIP_URL")
            .unwrap_or_else(|_| DEFAULT_SOURCES_ZIP_URL.to_string()),
    )
}

/// `RGSX_NATIVE_CATALOG=1` çağrılır. Katalog zaten mevcutsa no-op; değilse OTA'dan çeker.
/// Başarı/başarısızlık `bool` ile döner (başarısızsa native catalog yine de boş kalır,
/// eski davranış korunur).
pub async fn ensure_catalog_ready() -> bool {
    let data_dir = default_data_dir();
    if catalog_present(&data_dir) {
        return true;
    }

    info!(
        "native katalog verisi eksik ({}/systems_list.json + games/), OTA'dan indirilecek",
        data_dir.display()
    );

    let source = match resolve_zip_source() {
        Some(s) => s,
        None => {
            warn!("custom mod: kaynak ZIP bulunamadı (RGSX_SOURCES_ZIP_URL geçersiz/eksik)");
            return false;
        }
    };

    let is_http = source.starts_with("http://") || source.starts_with("https://");
    let zip_path = if is_http {
        let p = data_dir.join("data_download.zip");
        if !download(&source, &p).await {
            return false;
        }
        p
    } else {
        PathBuf::from(&source)
    };

    let ok = extract_zip(&zip_path, &data_dir);

    // Yalnızca bizim indirdiğimiz geçici dosyayı temizle (yerel custom zip'e dokunma).
    if is_http {
        let _ = std::fs::remove_file(&zip_path);
    }

    if ok {
        info!("native katalog verisi indirildi/çıkarıldı: {}", data_dir.display());
    } else {
        warn!("native katalog verisi kurulamadı: {}", data_dir.display());
    }
    ok
}

/// `reqwest` ile streaming indirme (Python `requests.get(stream=True)` eşleniği).
async fn download(url: &str, dest: &Path) -> bool {
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let client = match reqwest::Client::builder()
        .user_agent("Mozilla/5.0")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            warn!("HTTP istemcisi kurulamadı: {e}");
            return false;
        }
    };
    let resp = match client.get(url).send().await {
        Ok(r) => match r.error_for_status() {
            Ok(r) => r,
            Err(e) => {
                warn!("indirme HTTP hatası ({url}): {e}");
                return false;
            }
        },
        Err(e) => {
            warn!("indirme başarısız ({url}): {e}");
            return false;
        }
    };

    let mut file = match std::fs::File::create(dest) {
        Ok(f) => f,
        Err(e) => {
            warn!("indirme hedef dosyası oluşturulamadı: {e}");
            return false;
        }
    };

    let mut stream = resp.bytes_stream();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if let Err(e) = std::io::Write::write_all(&mut file, &bytes) {
                    warn!("indirme yazma hatası: {e}");
                    return false;
                }
            }
            Err(e) => {
                warn!("indirme akış hatası: {e}");
                return false;
            }
        }
    }
    true
}

/// `zip` crate ile çıkarma (Python `zipfile` eşleniği). Zip-slip korumalı.
fn extract_zip(zip_path: &Path, dest: &Path) -> bool {
    let file = match std::fs::File::open(zip_path) {
        Ok(f) => f,
        Err(e) => {
            warn!("kaynak ZIP açılamadı: {e}");
            return false;
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(e) => {
            warn!("kaynak ZIP okunamadı: {e}");
            return false;
        }
    };

    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.is_dir() {
            continue;
        }
        // Zip-slip koruması: enclosed_name `..` içeriyorsa None döner.
        let name = match entry.enclosed_name() {
            Some(p) => p.to_path_buf(),
            None => continue,
        };
        let out = dest.join(&name);
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut out_file = match std::fs::File::create(&out) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if std::io::copy(&mut entry, &mut out_file).is_err() {
            continue;
        }
        #[cfg(unix)]
        {
            use std::fs::Permissions;
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, Permissions::from_mode(0o644));
        }
    }
    true
}
