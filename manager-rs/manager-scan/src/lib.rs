//! Faz 12d — HDD tarama + gamelist.xml (EmulationStation) üretimi + history eşleme.
//!
//! Python `update_gamelist.py` / `update_gamelist_windows.py` + `utils/history_matches.py`
//! + `ROMS_FOLDER` tarama mantığının native portu. `manager-http` bu crate'i `/api/scan`
//! ve SSE `scan` olayı ile sunar.

pub mod disk;
pub mod gamelist;
pub mod history;
pub mod scan;

pub use scan::{PlatformScan, RomFile, ScanResult};

#[cfg(test)]
mod tests;
