use serde::{Deserialize, Serialize};
use std::env;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_discoverable")]
    pub discoverable: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            discoverable: true,
        }
    }
}

fn default_discoverable() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discoverable_defaults_to_true() {
        let config: Config = serde_json::from_str("{}").unwrap();
        assert!(config.discoverable);
        assert!(Config::default().discoverable);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub platform: String,
}

pub fn platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    }
}

pub fn platform_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "macOS"
    } else if cfg!(target_os = "windows") {
        "Windows"
    } else {
        "Linux"
    }
}

/// Best-effort, human friendly device name. Never fails: falls back to a
/// generic label so the app always has something to show on the network.
pub fn detect_default_name() -> String {
    let name = read_os_device_name()
        .or_else(|| env::var("COMPUTERNAME").ok())
        .or_else(|| env::var("HOSTNAME").ok())
        .unwrap_or_else(|| platform_label().to_string());

    clean_name(&name).unwrap_or_else(|| format!("{} device", platform_label()))
}

#[cfg(target_os = "macos")]
fn read_os_device_name() -> Option<String> {
    let output = Command::new("scutil")
        .args(["--get", "ComputerName"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

#[cfg(not(target_os = "macos"))]
fn read_os_device_name() -> Option<String> {
    None
}

pub fn clean_name(raw: &str) -> Option<String> {
    let value: String = raw
        .trim()
        .trim_end_matches(".local")
        .chars()
        .filter(|c| !c.is_control())
        .take(32)
        .collect();
    let value = value.trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}
