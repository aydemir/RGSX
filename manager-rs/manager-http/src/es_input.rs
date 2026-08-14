//! ES (EmulationStation) gamepad map'ini okuyup RGSX UI aksiyonlarına çevirir.
//!
//! TASK-005 — RetroBat/Batocera'daki `es_input.cfg`'yi launcher'dan bağımsız okur.
//! RGSX, kullanıcıya ikinci bir "Remap Controls" dayatmadan ES'in yazdığı aynı
//! fiziksel tuş haritasını UI navigasyonu için kullanır.
//!
//! Not: Mevcut webui tarayıcı Gamepad API (standart mapping) kullandığından,
//! ES custom remap'i tarayıcıda birebir yansımaz (browser SDL code'u expose etmez).
//! Bu modül ES map'ini parse edip sunar; tam custom-remap sadakati için native
//! SDL2 girdi yolu (TASK-005-B) gereklidir.

use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, serde::Serialize)]
pub struct EsInput {
    pub device_name: String,
    pub guid: String,
    /// ES aksiyon adı -> (tip, id, code)
    pub actions: HashMap<String, EsAction>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct EsAction {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: i64,
    pub code: i64,
}

/// ES aksiyon adı -> tarayıcı standart Gamepad index (SDL standart sıra = browser sıra).
/// Yalnızca varsayılan SDL mapping için birebirdir; ES custom remap tarayıcıda
/// tam yansımaz (browser Gamepad API SDL code'unu expose etmez).
pub fn es_action_to_gamepad_index(action: &str) -> Option<u8> {
    Some(match action {
        "a" => 0,
        "b" => 1,
        "x" => 2,
        "y" => 3,
        "pageup" => 4,
        "pagedown" => 5,
        "l2" => 6,
        "r2" => 7,
        "select" => 8,
        "start" => 9,
        "l3" => 10,
        "r3" => 11,
        "up" => 12,
        "down" => 13,
        "left" => 14,
        "right" => 15,
        _ => return None,
    })
}

/// `es_input.cfg` içeriğini parse eder -> controller listesi.
pub fn parse_es_input(contents: &str) -> Vec<EsInput> {
    let mut controllers: Vec<EsInput> = Vec::new();
    let mut current: Option<EsInput> = None;
    for raw in contents.lines() {
        let line = raw.trim();
        if line.starts_with("<inputConfig") {
            let device_name = attr(line, "deviceName").unwrap_or_default();
            let guid = attr(line, "deviceGUID").unwrap_or_default();
            current = Some(EsInput {
                device_name,
                guid,
                actions: HashMap::new(),
            });
        } else if line.starts_with("</inputConfig>") {
            if let Some(c) = current.take() {
                controllers.push(c);
            }
        } else if line.starts_with("<input ") {
            if let Some(name) = attr(line, "name") {
                let kind = attr(line, "type").unwrap_or_default();
                let id = attr(line, "id").and_then(|v| v.parse().ok()).unwrap_or(0);
                let code = attr(line, "code").and_then(|v| v.parse().ok()).unwrap_or(0);
                if let Some(c) = current.as_mut() {
                    c.actions.insert(name, EsAction { kind, id, code });
                }
            }
        }
    }
    controllers
}

fn attr(line: &str, key: &str) -> Option<String> {
    let pat = format!("{}=\"", key);
    let start = line.find(&pat)? + pat.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// `es_input.cfg` (veya `es_last_input.cfg`) dosyasını bulur.
/// Dönen `bool` = son kullanılan controller mı (`es_last_input.cfg`).
pub fn discover_es_input() -> Option<(PathBuf, bool)> {
    if let Ok(p) = env::var("RGSX_ES_INPUT") {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some((pb, false));
        }
    }
    // Batocera
    let candidates: Vec<(PathBuf, bool)> = vec![
        (
            PathBuf::from("/userdata/system/configs/emulationstation/es_last_input.cfg"),
            true,
        ),
        (
            PathBuf::from("/userdata/system/configs/emulationstation/es_input.cfg"),
            false,
        ),
    ];
    for (p, is_last) in candidates {
        if p.exists() {
            return Some((p, is_last));
        }
    }
    // RetroBat kök (env ile verilirse)
    if let Ok(root) = env::var("RGSX_RETROBAT_ROOT") {
        let p = Path::new(&root)
            .join("emulationstation")
            .join(".emulationstation")
            .join("es_input.cfg");
        if p.exists() {
            return Some((p, false));
        }
        let pl = Path::new(&root)
            .join("emulationstation")
            .join(".emulationstation")
            .join("es_last_input.cfg");
        if pl.exists() {
            return Some((pl, true));
        }
    }
    None
}

/// En iyi eşleşmeyi döndürür: `es_last_input.cfg` varsa onu, yoksa ilk controller.
pub fn load_best() -> Option<EsInput> {
    let (path, _is_last) = discover_es_input()?;
    let contents = std::fs::read_to_string(&path).ok()?;
    let mut all = parse_es_input(&contents);
    all.retain(|c| !c.actions.is_empty());
    all.into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"<inputList>
  <inputConfig type="joystick" deviceName="Xbox 360 Controller" deviceGUID="030000005e0400000000000000000000">
    <input name="a" type="button" id="0" value="1" code="292" />
    <input name="b" type="button" id="1" value="1" code="293" />
    <input name="up" type="button" id="12" value="1" code="304" />
    <input name="down" type="button" id="13" value="1" code="305" />
    <input name="start" type="button" id="9" value="1" code="301" />
    <input name="select" type="button" id="8" value="1" code="300" />
  </inputConfig>
</inputList>"#;

    #[test]
    fn parses_controllers_and_actions() {
        let v = parse_es_input(SAMPLE);
        assert_eq!(v.len(), 1);
        let c = &v[0];
        assert_eq!(c.device_name, "Xbox 360 Controller");
        assert_eq!(c.guid, "030000005e0400000000000000000000");
        assert_eq!(c.actions.get("a").unwrap().code, 292);
        assert_eq!(c.actions.get("up").unwrap().id, 12);
        // ES action -> tarayıcı standart index
        assert_eq!(es_action_to_gamepad_index("a"), Some(0));
        assert_eq!(es_action_to_gamepad_index("up"), Some(12));
        assert_eq!(es_action_to_gamepad_index("start"), Some(9));
    }
}
