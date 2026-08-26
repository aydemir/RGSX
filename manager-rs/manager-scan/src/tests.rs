//! `manager-scan` birim testleri (fixture tabanlı, offline).

#[cfg(test)]
mod tests {
    use crate::{disk, gamelist, history, scan};
    use std::io::Write;
    use std::path::Path;

    fn write(path: &Path, content: &[u8]) {
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).unwrap();
        }
        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(content).unwrap();
    }

    fn roms_fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        write(&root.join("nes/mario.zip"), b"dummy");
        write(&root.join("nes/zelda.zip"), b"dummy2");
        write(&root.join("snes/mario.sfc"), b"dummy3");
        write(&root.join("snes/sub/extra.zip"), b"dummy4");
        write(&root.join("docs/readme.txt"), b"not-a-rom");
        dir
    }

    #[test]
    fn scan_roms_groups_by_platform() {
        let dir = roms_fixture();
        let r = scan::scan_roms(dir.path());
        assert_eq!(r.total_files, 4);
        let nes = r.platforms.iter().find(|p| p.name == "nes").unwrap();
        assert_eq!(nes.files.len(), 2);
        assert_eq!(nes.total_bytes, 11);
        let snes = r.platforms.iter().find(|p| p.name == "snes").unwrap();
        assert_eq!(snes.files.len(), 2); // alt klasördeki zip dahil
        assert!(r.platforms.iter().all(|p| p.name != "docs"));
    }

    #[test]
    fn scan_missing_root_is_empty() {
        let r = scan::scan_roms(Path::new("/nonexistent/rgsx/roms"));
        assert_eq!(r.total_files, 0);
        assert!(r.platforms.is_empty());
    }

    #[test]
    fn gamelist_linux_writes_only_rgsx() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gamelist.xml");
        gamelist::write_rgsx_entry(&p, gamelist::GamelistVariant::Linux).unwrap();
        let games = gamelist::read_gamelist(&p).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(
            games[0].iter().find(|(k, _)| k == "path").unwrap().1,
            "./RGSX/RGSX.sh"
        );
        assert_eq!(
            games[0].iter().find(|(k, _)| k == "name").unwrap().1,
            "RGSX"
        );
    }

    #[test]
    fn gamelist_windows_merges_existing() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gamelist.xml");
        // Önce başka bir oyun yaz (merge=False ile linux tarzı tek entry).
        gamelist::write_gamelist(&p, &[("path", "./other/game.sh"), ("name", "Other")], false)
            .unwrap();
        // Sonra windows merge ile RGSX ekle.
        gamelist::write_rgsx_entry(&p, gamelist::GamelistVariant::Windows).unwrap();
        let games = gamelist::read_gamelist(&p).unwrap();
        assert_eq!(games.len(), 2);
        assert!(games
            .iter()
            .any(|g| g.iter().any(|(k, v)| k == "name" && v == "Other")));
        assert!(games.iter().any(|g| g
            .iter()
            .any(|(k, v)| k == "path" && v == "./RGSX Retrobat.bat")));
    }

    #[test]
    fn gamelist_windows_replaces_rgsx_entry() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("gamelist.xml");
        gamelist::write_rgsx_entry(&p, gamelist::GamelistVariant::Windows).unwrap();
        // Tekrar yaz → hâlâ 1 RGSX entry (eski silinir).
        gamelist::write_rgsx_entry(&p, gamelist::GamelistVariant::Windows).unwrap();
        let games = gamelist::read_gamelist(&p).unwrap();
        assert_eq!(games.len(), 1);
    }

    #[test]
    fn history_match_finds_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let roms = dir.path().to_path_buf();
        let file = roms.join("nes/mario.zip");
        write(&file, b"x");
        let entry = serde_json::json!({
            "local_path": file.to_string_lossy().to_string(),
            "platform": "nes",
            "local_filename": "mario.zip",
        });
        let m = history::match_history_local(&entry, &roms);
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].0, "mario.zip");
    }

    #[test]
    fn history_match_ignores_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let roms = dir.path().to_path_buf();
        let entry = serde_json::json!({ "local_path": "/nope/missing.zip" });
        let m = history::match_history_local(&entry, &roms);
        assert!(m.is_empty());
    }

    #[test]
    fn disk_usage_does_not_panic() {
        let _ = disk::disk_usage(std::env::temp_dir().as_path());
    }
}
