//! Windows Firewall uygulama kuralı yönetimi — `netsh advfirewall`.
//!
//! Python tarafında doğrudan karşılığı yok; roadmap'te HNetCfg COM / netsh önerilir.
//! Burada COM yerine `netsh advfirewall firewall add/delete/show rule` kullanılır
//! (basit, komut satırından doğrulanabilir). Kural ekleme/silme admin yetkisi
//! gerektirir; hata çağırana `FirewallError::Io` olarak yansır.

use std::path::Path;
use std::process::Command;

pub const RULE_NAME: &str = "RGSX Manager";

#[derive(Debug)]
pub enum FirewallError {
    /// netsh çalıştırılamadı veya çıkış kodu 0 değil.
    Io(String),
    /// Kural durumu sorgulanamadı (netsh çıktısı çözümlenemedi).
    Unknown,
}

impl std::fmt::Display for FirewallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FirewallError::Io(s) => write!(f, "netsh hatası: {s}"),
            FirewallError::Unknown => write!(f, "kural durumu çözümlenemedi"),
        }
    }
}

impl std::error::Error for FirewallError {}

fn run(args: &[&str]) -> Result<String, FirewallError> {
    let out = Command::new("netsh")
        .args(args)
        .output()
        .map_err(|e| FirewallError::Io(e.to_string()))?;
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if !out.status.success() {
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(FirewallError::Io(format!("({}): {}", out.status, detail.trim())));
    }
    Ok(stdout)
}

/// Kural adı (yerel sistemde dil bağımsız görünür) çıktıda var mı?
fn output_has_rule(output: &str) -> bool {
    // netsh çıktısında kural adı her iki (TR/EN) sistemde aynı görünür.
    output.contains(RULE_NAME)
}

/// `netsh advfirewall firewall add rule` ile uygulama kuralı ekler/günceller.
pub fn add_rule(exe_path: &Path) -> Result<(), FirewallError> {
    let exe = exe_path
        .to_str()
        .ok_or_else(|| FirewallError::Io("yol UTF-8 değil".into()))?;
    run(&[
        "advfirewall",
        "firewall",
        "add",
        "rule",
        &format!("name={RULE_NAME}"),
        "dir=in",
        "action=allow",
        &format!("program={exe}"),
        "enable=yes",
    ])?;
    Ok(())
}

/// `netsh advfirewall firewall delete rule` — kuralı kaldırır (yoksa başarısız olur).
pub fn remove_rule() -> Result<(), FirewallError> {
    run(&[
        "advfirewall",
        "firewall",
        "delete",
        "rule",
        &format!("name={RULE_NAME}"),
    ])?;
    Ok(())
}

/// Kural mevcut mu? (show çıktısında kural adını arar)
pub fn rule_exists() -> Result<bool, FirewallError> {
    let out = run(&[
        "advfirewall",
        "firewall",
        "show",
        "rule",
        &format!("name={RULE_NAME}"),
    ])?;
    Ok(output_has_rule(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_has_rule_detects_english() {
        let sample = "\nKural adı: RGSX Manager\nAktif: Evet\nYön: Gelen\nEylem: İzin Ver\n";
        assert!(output_has_rule(sample));
    }

    #[test]
    fn output_has_rule_false_when_absent() {
        let sample = "Aranan ölçütlerle eşleşen kural yok.\n";
        assert!(!output_has_rule(sample));
    }

    #[test]
    fn rule_name_is_stable() {
        assert_eq!(RULE_NAME, "RGSX Manager");
    }
}
