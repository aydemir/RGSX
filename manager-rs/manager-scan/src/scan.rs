//! `ROMS_FOLDER` özyinelemeli tarama — platform klasörlerine göre gruplama.

use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Tarama dahilindeki yaygın ROM/arşiv uzantıları.
pub const ROM_EXTENSIONS: &[&str] = &[
    "zip", "7z", "rar", "iso", "img", "bin", "cue", "gdi", "chd", "m3u", "rvz", "wbfs",
    "nes", "smc", "sfc", "fig", "gb", "gbc", "gba", "nds", "n64", "z64", "v64", "ws",
    "wsc", "pce", "sgx", "sms", "gg", "md", "gen", "smd", "a26", "a78", "col", "lnx",
    "jag", "iso", "cso", "psx", "ps", "pbp", "32x", "sfx", "fat", "Vec", "rom",
];

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RomFile {
    pub name: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlatformScan {
    pub name: String,
    pub folder: String,
    pub files: Vec<RomFile>,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ScanResult {
    pub root: String,
    pub platforms: Vec<PlatformScan>,
    pub total_bytes: u64,
    pub total_files: usize,
}

fn is_rom(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| ROM_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

/// `root` altındaki doğrudan alt klasörleri platform olarak alır, her birinde
/// ROM uzantılı dosyaları toplar (alt klasörlerdekiler de dahil).
pub fn scan_roms(root: &Path) -> ScanResult {
    let mut platforms: Vec<PlatformScan> = Vec::new();
    let mut total_bytes: u64 = 0;
    let mut total_files: usize = 0;

    if !root.is_dir() {
        return ScanResult {
            root: root.to_string_lossy().to_string(),
            platforms,
            total_bytes,
            total_files,
        };
    }

    for platform_dir in std::fs::read_dir(root)
        .map(|e| e.flatten().filter(|d| d.path().is_dir()).collect::<Vec<_>>())
        .unwrap_or_default()
    {
        let folder = platform_dir.file_name().to_string_lossy().to_string();
        let mut files = Vec::new();
        let mut ptotal: u64 = 0;
        for entry in WalkDir::new(platform_dir.path()).into_iter().flatten() {
            let p = entry.path();
            if p.is_file() && is_rom(p) {
                if let Ok(meta) = entry.metadata() {
                    let size = meta.len();
                    files.push(RomFile {
                        name: p
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default(),
                        size,
                        path: p.to_string_lossy().to_string(),
                    });
                    ptotal += size;
                    total_bytes += size;
                    total_files += 1;
                }
            }
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        // ROM içermeyen klasörler platform sayılmaz.
        if !files.is_empty() {
            platforms.push(PlatformScan {
                name: folder.clone(),
                folder,
                files,
                total_bytes: ptotal,
            });
        }
    }

    platforms.sort_by(|a, b| a.name.cmp(&b.name));
    ScanResult {
        root: root.to_string_lossy().to_string(),
        platforms,
        total_bytes,
        total_files,
    }
}

/// Tarama sonucunu insan-okunur özet metne çevirir (log/debug).
pub fn summarize(result: &ScanResult) -> String {
    format!(
        "{} platform, {} dosya, {:.2} MB",
        result.platforms.len(),
        result.total_files,
        result.total_bytes as f64 / 1_048_576.0
    )
}

/// `path` bir ROM dosyası mı? (extension kontrolü, dışarıdan kullanım).
pub fn is_rom_path(path: &Path) -> bool {
    is_rom(path)
}

#[allow(dead_code)]
fn _unused(_: PathBuf) {}
