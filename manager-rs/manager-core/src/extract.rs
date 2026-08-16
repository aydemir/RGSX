//! GAP-6 — Arşiv auto-extract / post-process (Rust port).
//!
//! Python kaynaklarıyla parity (birebir davranış):
//! - `utils/extract.py` (`extract_zip`/`extract_rar`/`extract_7z`) — arşiv açma.
//! - `network/helpers.py::_is_ps3_redump_target` — PS3 redump tespiti.
//! - `network/queue.py::download_rom` `force_extract` mantığı — ne zaman açılır.
//!
//! Kapsam:
//! - İndirme sonrası arşiv otomatik açma (BIOS, PS3 redump, `is_zip_non_supported`).
//! - Bozuk arşiv bütünlük testi (`zipfile.BadZipFile` parity — Madde B).
//!
//! Bilinçli kapsam dışı (ayrı/basamaklı):
//! - PS3 ISO şifre çözme (`handle_ps3`) — ağır, ayrı decryptor süb sistemi.
//!   `.iso` + PS3 → `ExtractError::Ps3DecryptUnsupported` (indirme başarısız
//!   sayılmaz; uyarı ile atlanır).
//! - RAR (`unrar`) — GPL/non-free lisans; Rust tarafında desteklenmez →
//!   `ExtractError::UnsupportedFormat`.

use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractError {
    #[error("io hatası ({path}): {source}")]
    Io { path: PathBuf, source: std::io::Error },
    #[error("desteklenmeyen arşiv formatı: {ext}")]
    UnsupportedFormat { ext: String },
    #[error("bozuk arşiv (bütünlük testi başarısız): {reason}")]
    CorruptArchive { reason: String },
    #[error("PS3 ISO şifre çözme bu modülde desteklenmiyor (ayrı decryptor gerekir)")]
    Ps3DecryptUnsupported,
    #[error("dosya bir arşiv değil veya bulunamadı")]
    NotAnArchive,
}

/// Çıkarma sonucu (informasyonel).
#[derive(Debug, Clone)]
pub struct ExtractOutcome {
    pub extracted_dir: PathBuf,
    pub extracted_files: usize,
    pub is_ps3_decrypt: bool,
}

/// `download_rom` çağrısından gelen extract ipucu (Python `platform` /
/// `platform_folder` / `is_zip_non_supported` + `get_auto_extract` parity).
#[derive(Debug, Clone, Default)]
pub struct ExtractHint {
    pub auto_extract: bool,
    pub is_zip_non_supported: bool,
    pub platform_folder: String,
    pub platform: String,
}

/// BIOS benzeri platform mu? (`queue.py` `bios_like` kümesi parity).
pub fn is_bios_platform(platform_folder: &str, platform: &str) -> bool {
    const BIOS_LIKE: &[&str] = &["BIOS", "- BIOS by TMCTV -", "- BIOS"];
    platform_folder.eq_ignore_ascii_case("bios")
        || BIOS_LIKE.iter().any(|b| platform.eq_ignore_ascii_case(b))
}

/// PS3 redump hedefi mi? (`helpers._is_ps3_redump_target` parity).
pub fn is_ps3_redump_target(platform_folder: &str, platform: &str) -> bool {
    const PS3: &[&str] = &["ps3", "PlayStation 3"];
    platform_folder.eq_ignore_ascii_case("ps3")
        || PS3.iter().any(|p| platform.eq_ignore_ascii_case(p))
}

/// `queue.py` `force_extract` mantığı (birebir parity):
/// ```text
/// force = is_zip_non_supported and auto_extract
/// if not force and auto_extract and bios: force = True
/// if not force and ps3_redump:           force = True   # PS3 auto_extract'tan bağımsız
/// ```
pub fn should_force_extract(
    auto_extract_enabled: bool,
    is_zip_non_supported: bool,
    platform_folder: &str,
    platform: &str,
) -> bool {
    let mut force = is_zip_non_supported && auto_extract_enabled;
    if !force && auto_extract_enabled && is_bios_platform(platform_folder, platform) {
        force = true;
    }
    if !force && is_ps3_redump_target(platform_folder, platform) {
        force = true;
    }
    force
}

/// Arşivi `dest_dir` içine açar. Desteklenen: `.zip`, `.7z`.
/// Diğer uzantılar için uygun `ExtractError` döner.
pub fn extract_archive(src: &Path, dest_dir: &Path) -> Result<ExtractOutcome, ExtractError> {
    if !src.is_file() {
        return Err(ExtractError::NotAnArchive);
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "zip" => extract_zip(src, dest_dir),
        "7z" => extract_7z(src, dest_dir),
        "rar" => Err(ExtractError::UnsupportedFormat { ext: "rar".into() }),
        "iso" => Err(ExtractError::Ps3DecryptUnsupported),
        _ => Err(ExtractError::NotAnArchive),
    }
}

/// Path-traversal koruması: çıktı yolu `dest_dir` içinde mi?
fn is_within(dest_dir: &Path, candidate: &Path) -> bool {
    candidate.starts_with(dest_dir)
}

fn extract_zip(src: &Path, dest_dir: &Path) -> Result<ExtractOutcome, ExtractError> {
    let file = std::fs::File::open(src)
        .map_err(|e| ExtractError::Io { path: src.to_path_buf(), source: e })?;
    // Bütünlük testi: merkezi dizin okunamazsa bozuk arşiv (BadZipFile parity).
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|e| ExtractError::CorruptArchive { reason: e.to_string() })?;

    std::fs::create_dir_all(dest_dir)
        .map_err(|e| ExtractError::Io { path: dest_dir.to_path_buf(), source: e })?;

    let mut count = 0usize;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| ExtractError::CorruptArchive { reason: e.to_string() })?;
        let name = match entry.enclosed_name() {
            Some(n) => n.to_path_buf(),
            None => continue, // güvensiz isim → atla
        };
        let out = dest_dir.join(&name);
        if !is_within(dest_dir, &out) {
            continue; // path traversal
        }
        if entry.is_dir() {
            std::fs::create_dir_all(&out)
                .map_err(|e| ExtractError::Io { path: out.clone(), source: e })?;
        } else {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| ExtractError::Io { path: parent.to_path_buf(), source: e })?;
            }
            let mut f = std::fs::File::create(&out)
                .map_err(|e| ExtractError::Io { path: out.clone(), source: e })?;
            std::io::copy(&mut entry, &mut f)
                .map_err(|e| ExtractError::Io { path: out.clone(), source: e })?;
            count += 1;
        }
    }
    Ok(ExtractOutcome {
        extracted_dir: dest_dir.to_path_buf(),
        extracted_files: count,
        is_ps3_decrypt: false,
    })
}

fn extract_7z(src: &Path, dest_dir: &Path) -> Result<ExtractOutcome, ExtractError> {
    let src_s = src
        .to_str()
        .ok_or_else(|| ExtractError::NotAnArchive)?;
    let dest_s = dest_dir
        .to_str()
        .ok_or_else(|| ExtractError::NotAnArchive)?;

    std::fs::create_dir_all(dest_dir)
        .map_err(|e| ExtractError::Io { path: dest_dir.to_path_buf(), source: e })?;

    // Çıkarma öncesi dosya sayısı (diff ile yeni açılanları sayarız).
    let before = count_files(dest_dir);
    // Bütünlük testi + çıkarma: bozuk arşivde sevenz hata verir (BadZipFile parity).
    sevenz_rust::decompress_file(src_s, dest_s)
        .map_err(|e| ExtractError::CorruptArchive { reason: e.to_string() })?;
    let after = count_files(dest_dir);
    Ok(ExtractOutcome {
        extracted_dir: dest_dir.to_path_buf(),
        extracted_files: after.saturating_sub(before),
        is_ps3_decrypt: false,
    })
}

fn count_files(dir: &Path) -> usize {
    let mut n = 0usize;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                n += count_files(&p);
            } else {
                n = n.saturating_add(1);
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn tmp_dir(label: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rgsx_extract_test_{}_{}", label, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    fn make_zip(path: &Path, entries: &[(&str, &[u8])]) {
        use zip::write::SimpleFileOptions;
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let opts = SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (name, data) in entries {
            writer.start_file(*name, opts).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap();
    }

    #[test]
    fn corrupt_zip_returns_corrupt_archive() {
        let dir = tmp_dir("corrupt");
        let zip = dir.join("bad.zip");
        write_file(&zip, b"this is not a zip file at all");
        let out = dir.join("out");
        let res = extract_archive(&zip, &out);
        assert!(matches!(res, Err(ExtractError::CorruptArchive { .. })), "got: {res:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn valid_zip_extracts_files() {
        let dir = tmp_dir("valid");
        let zip = dir.join("game.zip");
        make_zip(&zip, &[("roms/foo.bin", b"AAAA"), ("readme.txt", b"BBBB")]);
        let out = dir.join("out");
        let res = extract_archive(&zip, &out).expect("extract ok");
        assert_eq!(res.extracted_files, 2);
        assert!(out.join("roms/foo.bin").is_file());
        assert!(out.join("readme.txt").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_traversal_is_rejected() {
        let dir = tmp_dir("traversal");
        let zip = dir.join("evil.zip");
        // "../escape.txt" güvensiz isim → enclosed_name None → atlanır.
        make_zip(&zip, &[("../escape.txt", b"X")]);
        let out = dir.join("out");
        let res = extract_archive(&zip, &out).expect("extract runs");
        assert_eq!(res.extracted_files, 0);
        assert!(!dir.join("escape.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn detection_rules() {
        assert!(is_bios_platform("bios", "anything"));
        assert!(is_bios_platform("roms", "- BIOS by TMCTV -"));
        assert!(!is_bios_platform("ps3", "playstation 3"));

        assert!(is_ps3_redump_target("ps3", "x"));
        assert!(is_ps3_redump_target("roms", "PlayStation 3"));
        assert!(!is_ps3_redump_target("bios", "bios"));
    }

    #[test]
    fn should_force_extract_truth_table() {
        // is_zip_non_supported yalnızca auto_extract ile force eder
        assert!(should_force_extract(true, true, "roms", "x"));
        assert!(!should_force_extract(false, true, "roms", "x"));
        // BIOS + auto_extract → force
        assert!(should_force_extract(true, false, "bios", "x"));
        assert!(!should_force_extract(false, false, "bios", "x"));
        // PS3 redump → auto_extract'tan bağımsız force
        assert!(should_force_extract(false, false, "ps3", "x"));
        assert!(should_force_extract(true, false, "roms", "PlayStation 3"));
        // normal platform, auto_extract açık, arşiv değil → force DEĞİL
        assert!(!should_force_extract(true, false, "snes", "Super Nintendo"));
    }

    #[test]
    fn unsupported_formats() {
        let dir = tmp_dir("unsupported");
        let rar = dir.join("x.rar");
        write_file(&rar, b"rar-data");
        assert!(matches!(
            extract_archive(&rar, &dir.join("o")),
            Err(ExtractError::UnsupportedFormat { ext: _ })
        ));
        let iso = dir.join("x.iso");
        write_file(&iso, b"iso-data");
        assert!(matches!(
            extract_archive(&iso, &dir.join("o2")),
            Err(ExtractError::Ps3DecryptUnsupported)
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
