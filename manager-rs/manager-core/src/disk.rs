//! Disk alanı / yazılabilirlik ön-kontrolü (Python `_ensure_sufficient_disk_space`
//! + `os.access(dest_dir, os.W_OK)` portu). İndirme başlamadan önce hedefin
//! yazılabilir ve yetecek kadar boş alanı olduğuna emin olur.

use std::io::Write;
use std::path::Path;
use sysinfo::Disks;

/// Disk/yazma ön-kontrolü hatası.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskError {
    /// Hedef dizine yazma izni yok (Python `PermissionError`).
    PermissionDenied(String),
    /// Yetersiz disk alanı (Python `InsufficientDiskSpaceError`).
    InsufficientSpace { free: u64, required: u64 },
    /// Disk sorgusu başarısız (bağlanamadı vb.) — çağıran "atla/devam et" kararı verir
    /// (Python parity: free_bytes None → kontrolü atla).
    QueryFailed(String),
}

impl std::fmt::Display for DiskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DiskError::PermissionDenied(m) => write!(f, "yazma izni yok: {m}"),
            DiskError::InsufficientSpace { free, required } => {
                write!(
                    f,
                    "disk alanı yetersiz: gerekli {required} bayt, mevcut {free} bayt"
                )
            }
            DiskError::QueryFailed(m) => write!(f, "disk sorgusu başarısız: {m}"),
        }
    }
}

impl std::error::Error for DiskError {}

/// `path`'i içeren diskin kullanılabilir (non-root) boş bayt sayısını döndürür.
/// Bulunamazsa `QueryFailed`.
pub fn free_disk_bytes(path: &Path) -> Result<u64, DiskError> {
    let disks = Disks::new_with_refreshed_list();
    let disk = disks
        .iter()
        .find(|d| path.starts_with(d.mount_point()))
        .or_else(|| {
            disks
                .iter()
                .filter(|d| path.starts_with(d.mount_point()))
                .max_by_key(|d| d.mount_point().as_os_str().len())
        });
    match disk {
        Some(d) => Ok(d.available_space()),
        None => Err(DiskError::QueryFailed(format!(
            "disk bulunamadı: {}",
            path.display()
        ))),
    }
}

/// Hedef dizine gerçekten yazılabildiğini probe ile doğrular (Python `os.access`'ten
/// daha güvenilir: os.access Windows'ta yanıltıcı olabilir). Dizin yoksa oluşturur.
pub fn ensure_writable(dest_dir: &Path) -> Result<(), DiskError> {
    if let Err(e) = std::fs::create_dir_all(dest_dir) {
        if e.kind() == std::io::ErrorKind::PermissionDenied {
            return Err(DiskError::PermissionDenied(format!(
                "dizin oluşturulamadı: {}",
                dest_dir.display()
            )));
        }
        // Diğer oluşturma hataları probe ile anlaşılır kalsın.
    }
    let probe = dest_dir.join(format!(".rgsx_write_probe_{}.tmp", std::process::id()));
    match std::fs::File::create(&probe)
        .and_then(|mut f| f.write_all(b"x").and_then(|_| f.sync_all()))
    {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let _ = std::fs::remove_file(&probe);
            Err(DiskError::PermissionDenied(format!(
                "yazma izni yok: {}",
                dest_dir.display()
            )))
        }
        Err(e) => Err(DiskError::QueryFailed(format!(
            "yazma probe hatası ({}): {e}",
            dest_dir.display()
        ))),
    }
}

/// İndirme öncesi birleşik ön-kontrol: önce yazılabilirlik (probe), sonra
/// `required > 0` ise alan kontrolü. `required == 0` alan kontrolü atlanır
/// (bilinmeyen boyut — Python parity).
pub fn precheck_destination(dest_dir: &Path, required: u64) -> Result<(), DiskError> {
    ensure_writable(dest_dir)?;
    if required > 0 {
        let free = free_disk_bytes(dest_dir)?;
        if free < required {
            return Err(DiskError::InsufficientSpace { free, required });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dir_writable_and_has_space() {
        let dir = std::env::temp_dir();
        let res = precheck_destination(&dir, 0);
        assert!(matches!(res, Ok(())) || matches!(res, Err(DiskError::QueryFailed(_))));
    }

    #[test]
    fn probe_file_cleaned_up() {
        let dir = std::env::temp_dir();
        let _ = precheck_destination(&dir, 0);
        let probe = dir.join(format!(".rgsx_write_probe_{}.tmp", std::process::id()));
        assert!(!probe.exists(), "probe dosyası silinmeli");
    }

    #[test]
    fn impossible_required_space_reports_insufficient() {
        let dir = std::env::temp_dir();
        // 1 EiB istemek pratikte her zaman yetersiz (query başarısızsa atlanır).
        match precheck_destination(&dir, u64::MAX) {
            Err(DiskError::InsufficientSpace { .. }) => {}
            Err(DiskError::QueryFailed(_)) => {}
            other => panic!("beklenmeyen sonuç: {other:?}"),
        }
    }
}
