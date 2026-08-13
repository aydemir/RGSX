//! Disk kullanımı (`config.get_disk_usage` / `utils.get_disk_usage` portu).

use std::path::Path;
use sysinfo::Disks;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiskUsage {
    pub total: u64,
    pub used: u64,
    pub free: u64,
}

/// `path` içeren diskin toplam/kullanılan/boş baytlarını döndürür.
/// Bulunamazsa (0,0,0) döner.
pub fn disk_usage(path: &Path) -> DiskUsage {
    let mut disks = Disks::new_with_refreshed_list();
    let disk = disks
        .iter()
        .find(|d| path.starts_with(d.mount_point()))
        .or_else(|| {
            // En spesifik mount point'i bul (varsayılan fallback).
            disks
                .iter()
                .filter(|d| path.starts_with(d.mount_point()))
                .max_by_key(|d| d.mount_point().as_os_str().len())
        });
    match disk {
        Some(d) => {
            let total = d.total_space();
            let available = d.available_space();
            let free = available;
            let used = total.saturating_sub(available);
            DiskUsage { total, used, free }
        }
        None => DiskUsage { total: 0, used: 0, free: 0 },
    }
}

/// `path` bir diskin kökü mü? (Mount noktası kontrolü, tarama dışlama için).
#[allow(dead_code)]
pub fn is_mount_point(path: &Path) -> bool {
    let disks = Disks::new_with_refreshed_list();
    disks.iter().any(|d| d.mount_point() == path)
}
