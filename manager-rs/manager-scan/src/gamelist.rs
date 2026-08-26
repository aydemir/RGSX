//! `gamelist.xml` (EmulationStation) okuma/yazma — `update_gamelist.py` (Linux) ve
//! `update_gamelist_windows.py` (Windows) portu. Linux RGSX entry'sini dosyaya tek
//! başına yazar (diğer oyunları korumaz); Windows mevcut entry'leri koruyup RGSX
//! entry'sini merge eder (ES yönetilen alanları korur).

use std::path::Path;

/// Bir `<game>` öğesinin sıralı alanları (child elementler).
pub type GameFields = Vec<(String, String)>;

/// Linux RGSX gamelist entry'si (Retrobat.sh).
pub const RGSX_ENTRY_LINUX: &[(&str, &str)] = &[
    ("path", "./RGSX/RGSX.sh"),
    ("name", "RGSX"),
    ("desc", "Retro Games Sets X - Games Downloader"),
    ("image", "./images/RGSX.png"),
    ("video", "./videos/RGSX.mp4"),
    ("marquee", "./images/RGSX.png"),
    ("thumbnail", "./images/RGSX.png"),
    ("fanart", "./images/RGSX.png"),
    ("rating", "1"),
    ("releasedate", "20250620T165718"),
    ("developer", "RetroGameSets.fr"),
    ("genre", "Various / Utilities"),
];

/// Windows RGSX gamelist entry'si (RGSX rust.bat — gap-02: eski Retrobat.bat
/// Python launcher'ı silindi, native launcher aynı dizinde).
pub const RGSX_ENTRY_WINDOWS: &[(&str, &str)] = &[
    ("path", "./RGSX rust.bat"),
    ("name", "RGSX"),
    ("desc", "Retro Games Sets X - Games Downloader"),
    ("image", "./images/RGSX.png"),
    ("video", "./videos/RGSX.mp4"),
    ("marquee", "./images/RGSX.png"),
    ("thumbnail", "./images/RGSX.png"),
    ("fanart", "./images/RGSX.png"),
    ("releasedate", "20250620T165718"),
    ("developer", "RetroGameSets.fr"),
    ("genre", "Various / Utilities"),
];

/// Hangi platform için yazım yapılacağı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamelistVariant {
    Linux,
    Windows,
}

fn to_fields(entry: &[(&str, &str)]) -> GameFields {
    entry
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

/// `gamelist.xml` dosyasını parse eder → her oyunun alan listesi.
pub fn read_gamelist(path: &Path) -> Result<Vec<GameFields>, String> {
    let mut reader = quick_xml::reader::Reader::from_file(path).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let mut games: Vec<GameFields> = Vec::new();
    let mut cur: Option<GameFields> = None;
    let mut cur_key: Option<String> = None;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(quick_xml::events::Event::Start(e)) => {
                let name = String::from_utf8_lossy(e.name().into_inner()).to_string();
                if name == "game" {
                    cur = Some(Vec::new());
                } else if cur.is_some() {
                    cur_key = Some(name);
                }
            }
            Ok(quick_xml::events::Event::Text(t)) => {
                if let (Some(cur), Some(key)) = (&mut cur, &cur_key) {
                    let val = t.unescape().map(|c| c.to_string()).unwrap_or_default();
                    cur.push((key.clone(), val));
                }
            }
            Ok(quick_xml::events::Event::End(e)) => {
                let name = String::from_utf8_lossy(e.name().into_inner()).to_string();
                if name == "game" {
                    if let Some(g) = cur.take() {
                        games.push(g);
                    }
                    cur_key = None;
                } else {
                    cur_key = None;
                }
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Err(e) => return Err(e.to_string()),
            _ => {}
        }
        buf.clear();
    }
    Ok(games)
}

/// `gamelist.xml` yazar. `merge=true` (Windows) ise mevcut entry'ler korunur ve
/// RGSX path'li eski entry silinip yenisi eklenir; `merge=false` (Linux) ise dosya
/// yalnızca RGSX entry'sinden oluşur.
pub fn write_gamelist(path: &Path, entry: &[(&str, &str)], merge: bool) -> Result<(), String> {
    let entry_path = entry
        .iter()
        .find(|(k, _)| *k == "path")
        .map(|(_, v)| v.to_string());
    let mut games: Vec<GameFields> = if merge {
        read_gamelist(path)
            .unwrap_or_default()
            .into_iter()
            .filter(|g| match &entry_path {
                Some(ep) => g.iter().all(|(k, v)| !(*k == "path" && v == ep)),
                None => true,
            })
            .collect()
    } else {
        Vec::new()
    };
    games.push(to_fields(entry));

    let mut out = Vec::new();
    let mut w = quick_xml::Writer::new(&mut out);
    w.write_event(quick_xml::events::Event::Start(
        quick_xml::events::BytesStart::new("gameList"),
    ))
    .map_err(|e| e.to_string())?;
    for game in &games {
        w.write_event(quick_xml::events::Event::Start(
            quick_xml::events::BytesStart::new("game"),
        ))
        .map_err(|e| e.to_string())?;
        for (k, v) in game {
            w.write_event(quick_xml::events::Event::Start(
                quick_xml::events::BytesStart::new(k.as_str()),
            ))
            .map_err(|e| e.to_string())?;
            w.write_event(quick_xml::events::Event::Text(
                quick_xml::events::BytesText::new(v.as_str()),
            ))
            .map_err(|e| e.to_string())?;
            w.write_event(quick_xml::events::Event::End(
                quick_xml::events::BytesEnd::new(k.as_str()),
            ))
            .map_err(|e| e.to_string())?;
        }
        w.write_event(quick_xml::events::Event::End(
            quick_xml::events::BytesEnd::new("game"),
        ))
        .map_err(|e| e.to_string())?;
    }
    w.write_event(quick_xml::events::Event::End(
        quick_xml::events::BytesEnd::new("gameList"),
    ))
    .map_err(|e| e.to_string())?;

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(path, &out).map_err(|e| e.to_string())?;
    Ok(())
}

/// Uygun variant ile RGSX entry'sini yazar.
pub fn write_rgsx_entry(path: &Path, variant: GamelistVariant) -> Result<(), String> {
    match variant {
        GamelistVariant::Linux => write_gamelist(path, RGSX_ENTRY_LINUX, false),
        GamelistVariant::Windows => write_gamelist(path, RGSX_ENTRY_WINDOWS, true),
    }
}
