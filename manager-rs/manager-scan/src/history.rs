//! History eşleme — `utils/history_matches.py::get_existing_history_matches` portu.
//! Bir history entry'sinin `local_path` / `moved_paths` içindeki hâlâ mevcut
//! dosyaları döndürür (HDD'de neyin durduğunu belirler).

use std::path::Path;

/// `(dosya_adı, mutlak_yol)` çiftleri — hâlâ diskte olanlar.
pub fn match_history_local(entry: &serde_json::Value, roms_folder: &Path) -> Vec<(String, String)> {
    let empty = serde_json::Value::Null;
    let mut candidates: Vec<String> = Vec::new();

    if let Some(lp) = entry.get("local_path").and_then(|v| v.as_str()) {
        if !lp.is_empty() {
            candidates.push(lp.to_string());
        }
    }
    if let Some(arr) = entry.get("moved_paths").and_then(|v| v.as_array()) {
        for v in arr {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    candidates.push(s.to_string());
                }
            }
        }
    }

    // `local_filename` + platform klasörü tabanlı aday da ekle.
    if let (Some(lf), Some(plat)) = (
        entry.get("local_filename").and_then(|v| v.as_str()),
        entry.get("platform").and_then(|v| v.as_str()),
    ) {
        if !lf.is_empty() && !plat.is_empty() {
            candidates.push(roms_folder.join(plat).join(lf).to_string_lossy().to_string());
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut matches = Vec::new();
    for c in candidates {
        let p = Path::new(&c).to_path_buf();
        let norm = p.to_string_lossy().to_string();
        if !seen.insert(norm.clone()) {
            continue;
        }
        if p.is_file() {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            matches.push((name, norm));
        }
    }
    matches
}

/// Birden çok entry için toplu eşleme (YAML'daki `history` listesi).
pub fn match_history_all(
    history: &[serde_json::Value],
    roms_folder: &Path,
) -> Vec<(serde_json::Value, Vec<(String, String)>)> {
    history
        .iter()
        .map(|e| (e.clone(), match_history_local(e, roms_folder)))
        .collect()
}
