//! Auto-start: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run\RGSXManager`.
//!
//! Python karşılığı: `rgsx_manager.py` `autostart_install/remove/is_autostart_enabled`.
//! Değer REG_SZ, komut `"<exe>" "<script>" --minimized` biçiminde.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use windows::core::PCWSTR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, REG_VALUE_TYPE,
};

pub const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
pub const VALUE_NAME: &str = "RGSXManager";

/// Geniş (UTF-16, null-terminated) dizi üretir — registry PCWSTR için.
fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(Some(0)).collect()
}

/// `"<exe>" "<script>" --minimized` biçiminde autostart komutu üretir.
pub fn command(exe: &str, script: &str) -> String {
    format!("\"{exe}\" \"{script}\" --minimized")
}

/// Kendi exe'sini `--minimized` ile autostart komutu olarak üretir (Rust binary).
pub fn command_self() -> String {
    let exe = std::env::current_exe()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "manager-bin.exe".to_string());
    format!("\"{exe}\" --minimized")
}

/// Registry'de autostart değeri var mı?
pub fn is_enabled() -> bool {
    let name = wide(VALUE_NAME);
    let key = wide(RUN_KEY);
    let mut hkey = HKEY::default();
    let mut ty = REG_VALUE_TYPE::default();
    let mut size = 0u32;
    unsafe {
        let ok = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            KEY_QUERY_VALUE,
            &mut hkey,
        );
        if ok.is_err() {
            return false;
        }
        let present = RegQueryValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            None,
            Some(&mut ty),
            None,
            Some(&mut size),
        )
        .is_ok();
        let _ = RegCloseKey(hkey);
        present
    }
}

/// Registry'ye REG_SZ olarak autostart değerini yazar.
pub fn install(cmd: &str) -> std::io::Result<()> {
    let name = wide(VALUE_NAME);
    let key = wide(RUN_KEY);
    let data = wide(cmd); // null-terminated UTF-16
    let bytes = data.as_ptr() as *const u8;
    let len = (data.len() * 2) as u32; // UTF-16 bayt uzunluğu (null dahil)
    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .map_err(w32_err)?;
        let r = RegSetValueExW(
            hkey,
            PCWSTR(name.as_ptr()),
            0,
            REG_SZ,
            Some(std::slice::from_raw_parts(bytes, len as usize)),
        );
        let _ = RegCloseKey(hkey);
        r.map_err(w32_err)
    }
}

/// Registry'deki autostart değerini siler (yoksa idempotent başarı).
pub fn remove() -> std::io::Result<()> {
    let name = wide(VALUE_NAME);
    let key = wide(RUN_KEY);
    unsafe {
        let mut hkey = HKEY::default();
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key.as_ptr()),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        )
        .map_err(w32_err)?;
        let r = RegDeleteValueW(hkey, PCWSTR(name.as_ptr()));
        let _ = RegCloseKey(hkey);
        r.map_err(w32_err)
    }
}

fn w32_err(e: windows::core::Error) -> std::io::Error {
    std::io::Error::from_raw_os_error(e.code().0 as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_builds_quoted_minimized() {
        let c = command("C:\\Python\\pythonw.exe", r"C:\RGSX\rgsx_manager.py");
        assert_eq!(
            c,
            "\"C:\\Python\\pythonw.exe\" \"C:\\RGSX\\rgsx_manager.py\" --minimized"
        );
    }

    #[test]
    fn command_self_minimized() {
        let c = command_self();
        assert!(c.starts_with('"'));
        assert!(c.ends_with("\" --minimized"), "komut: {c}");
    }

    #[test]
    fn value_name_is_stable() {
        // Registry değer adı Python ile aynı olmalı (mevcut kurulumları bozma).
        assert_eq!(VALUE_NAME, "RGSXManager");
    }
}
