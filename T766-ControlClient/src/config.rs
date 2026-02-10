use std::fs;
use std::path::PathBuf;
use toml;
use crate::host::{os, user};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClientConfig {
    pub primary_url: String,
    pub fallback_url: String,
}

fn conf_path() -> PathBuf {
    let user = user();
    if os() == "windows" {
        PathBuf::from(format!(r"C:\Users\{user}\AppData\Local\T766 Control System\settings.toml"))
    } else {
        PathBuf::from("/etc/t766/settings.toml")
    }
}
pub fn load_config() -> Result<ClientConfig, String> {
    let path = conf_path();
    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {:?}: {}", path, e))?;

    toml::from_str(&contents)
        .map_err(|e| format!("Invalid config {:?}: {}", path, e))
}