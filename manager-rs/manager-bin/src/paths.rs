//! gap-26 — manager-bin path-resolution.
//!
//! Tüm path env'leri (`RGSX_WEBUI_DIR`, `RGSX_DATA_DIR`, `RGSX_LANGUAGES_FOLDER`,
//! `RGSX_DOWNLOADS_FOLDER`, `RGSX_LOGS_FOLDER`, `RGSX_MANAGER_SCRIPT`) `current_exe()`
//! konumundan türetilir. Desen: **env varsa öncelik ver, yoksa exe'den türet, panic ETME.**
//!
//! `RGSX_ROOT` Rust'ta OKUNMUYOR → set EDİLMEZ (gap-26 Karar).
//!
//! ZORUNLU SIRA: `resolve_paths()` yalnız `main()`'in EN BAŞINDA, tokio runtime / herhangi
//! bir thread spawn EDİLMENDEN ÖNCE çağrılır. `std::env::set_var` thread-safe DEĞİL (Rust
//! 1.80+ `unsafe`) — burada tek thread, runtime öncesi garantisi vardır.

use std::path::{Path, PathBuf};

/// Türetilen RetroBat/RGSX yolları. `root` = `roms/` içeren RetroBat kökü;
/// `rgsx_dir` = `root/roms/ports/RGSX`.
#[derive(Debug, Clone)]
pub struct RgsxPaths {
    pub root: PathBuf,
    pub rgsx_dir: PathBuf,
    pub webui_dir: PathBuf,
    pub data_dir: PathBuf,
    pub languages_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub manager_script: PathBuf,
}

/// `current_exe()`'den yukarı çıkarak `roms/ports/RGSX` imzasını arar.
/// Bulunursa o ata = RetroBat root. Bulunamazsa 3×`.parent()` fallback + `tracing::warn!`.
///
/// Döner: `(root, rgsx_dir)`.
fn find_anchor() -> (PathBuf, PathBuf) {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Anchor: bir atada `roms/ports/RGSX` alt dizini varsa o ata = RetroBat root.
    let mut cand = dir.clone();
    let mut root = None;
    while cand.parent().is_some() {
        let signature = cand.join("roms").join("ports").join("RGSX");
        if signature.is_dir() {
            root = Some(cand.clone());
            break;
        }
        match cand.parent() {
            Some(p) => cand = p.to_path_buf(),
            None => break,
        }
    }

    let root = match root {
        Some(r) => r,
        None => {
            // Fallback: exe dizininden 3×`.parent()` (windows/RGSX/../../.. == RetroBat root).
            let fb = dir
                .parent()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .unwrap_or(&dir)
                .to_path_buf();
            tracing::warn!(
                "path anchor (roms/ports/RGSX) bulunamadı, fallback (.parent×3) kullanılıyor: {}",
                fb.display()
            );
            fb
        }
    };

    // Geçici debug-doğrulama (gap-26): eski `.bat` ROOT_DIR hesabı (SCRIPT_DIR\..\..,
    // yani exe'den 3×.parent) ile anchor root'u karşılaştır. Fark → çift-`roms` şüphesi.
    // Normal RetroBat yerleşiminde ikisi eşit olur; yalnız relocate senaryosunda uyarı düşer.
    let legacy_root = dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .unwrap_or(&dir)
        .to_path_buf();
    if legacy_root != root {
        tracing::warn!(
            "anchor/legacy root uyumsuzluğu (çift-roms şüphesi): anchor={} legacy={}",
            root.display(),
            legacy_root.display()
        );
    } else {
        tracing::debug!("path anchor root eşleşti: {}", root.display());
    }

    let rgsx_dir = root.join("roms").join("ports").join("RGSX");
    (root, rgsx_dir)
}

/// Env varsa onu Path'e çevir; yoksa `derived`'ı kullan.
fn env_or(derived: &Path, key: &str) -> PathBuf {
    match std::env::var(key) {
        Ok(v) if !v.is_empty() => PathBuf::from(v),
        _ => derived.to_path_buf(),
    }
}

/// Türetilen yolu env'e geri yazar — yalnızca override YOKSA.
/// SAFETY: `resolve_paths()` tek thread'de, tokio runtime başlamadan önce çağrılır.
fn apply(key: &str, val: &Path) {
    if std::env::var_os(key).is_none() {
        let s = val.to_string_lossy().to_string();
        unsafe { std::env::set_var(key, s) };
    }
}

/// gap-26 — path-resolution: exe'den türet + env override. `main()` EN BAŞINDA çağrılır.
pub fn resolve_paths() -> RgsxPaths {
    let (root, rgsx_dir) = find_anchor();

    let webui_dir = env_or(&rgsx_dir.join("webui"), "RGSX_WEBUI_DIR");
    let data_dir = env_or(
        &root.join("saves").join("ports").join("rgsx"),
        "RGSX_DATA_DIR",
    );
    let languages_dir = env_or(&rgsx_dir.join("languages"), "RGSX_LANGUAGES_FOLDER");
    let downloads_dir = env_or(&data_dir.join("downloads"), "RGSX_DOWNLOADS_FOLDER");
    let logs_dir = env_or(&data_dir.join("logs"), "RGSX_LOGS_FOLDER");
    let manager_script = env_or(&rgsx_dir.join("qbittorrent_backend.py"), "RGSX_MANAGER_SCRIPT");

    // Downstream crates (catalog/settings/api) bu env'leri okur → geri yaz.
    apply("RGSX_WEBUI_DIR", &webui_dir);
    apply("RGSX_DATA_DIR", &data_dir);
    apply("RGSX_LANGUAGES_FOLDER", &languages_dir);
    apply("RGSX_DOWNLOADS_FOLDER", &downloads_dir);
    apply("RGSX_LOGS_FOLDER", &logs_dir);
    apply("RGSX_MANAGER_SCRIPT", &manager_script);

    RgsxPaths {
        root,
        rgsx_dir,
        webui_dir,
        data_dir,
        languages_dir,
        downloads_dir,
        logs_dir,
        manager_script,
    }
}
