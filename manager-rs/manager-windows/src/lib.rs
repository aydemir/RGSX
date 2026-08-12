//! manager-windows: Windows-only bileşenler.
//!
//! - `autostart` — HKCU Run registry kaydı (`RGSXManager`)
//! - `firewall` — netsh ile uygulama kuralı yönetimi
//! - `tray` — tray-icon + muda tabanlı sistem tepsisi ikonu + menü
//!
//! Tümü `cfg(windows)`; diğer platformlarda crate boş derlenir.

#![cfg(windows)]

pub mod autostart;
pub mod firewall;
pub mod tray;
