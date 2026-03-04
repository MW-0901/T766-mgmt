use std::fs;
use std::path::PathBuf;
use toml;
use crate::host::os;
use serde::Deserialize;
use std::io::Write;
use log::info;

#[derive(Deserialize)]
pub struct ClientConfig {
    pub primary_url: String,
    pub fallback_url: String,
}

macro_rules! conf_file {
    ($filename:expr) => {
        if os() == "windows" {
            PathBuf::from(format!(r"C:\ProgramData\T766 Control System\{}", $filename))
        } else {
            PathBuf::from(format!("/etc/t766/{}", $filename))
        }
    };
}

pub fn log_path() -> PathBuf {
    conf_file!("client.log")
}

fn conf_path() -> PathBuf {
    conf_file!("settings.toml")
}

pub fn checkin_logs() -> Result<Vec<String>, Vec<String>> {
    let path = conf_file!("checkin-logs");
    let file = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read {:?}: {}", path, e));
    match file {
        Ok(file) => {
            let logs = file.split("\n\n\n")
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            info!("Logs: {:?}", logs);
            Ok(logs)
        }
        Err(e) => Err(vec![e.to_string()]),
    }
}

pub fn clear_logs() -> std::io::Result<()> {
    let path = conf_file!("checkin-logs");
    let old_path = conf_file!("old-checkin-logs");

    if let Ok(contents) = fs::read_to_string(&path) {
        let mut old_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&old_path)?;

        writeln!(old_file, "{}", contents)?;
    }

    fs::write(&path, "")
}

pub fn load_config() -> Result<ClientConfig, String> {
    let programdata = conf_path();
    if programdata.exists() {
        let contents = fs::read_to_string(&programdata)
            .map_err(|e| format!("Failed to read {:?}: {}", programdata, e))?;
        return toml::from_str(&contents)
            .map_err(|e| format!("Invalid config: {}", e));
    }

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_default();
    let exe_sibling = exe_dir.join("settings.toml");
    if exe_sibling.exists() {
        let contents = fs::read_to_string(&exe_sibling)
            .map_err(|e| format!("Failed to read {:?}: {}", exe_sibling, e))?;
        return toml::from_str(&contents)
            .map_err(|e| format!("Invalid config: {}", e));
    }

    if os() == "windows" {
        let installed = PathBuf::from(r"C:\Program Files\T766 Control System\settings.toml");
        if installed.exists() {
            let contents = fs::read_to_string(&installed)
                .map_err(|e| format!("Failed to read {:?}: {}", installed, e))?;
            return toml::from_str(&contents)
                .map_err(|e| format!("Invalid config: {}", e));
        }
    }

    Err(format!(
        "Could not find settings.toml in any of: {:?}, {:?}, or installed Program Files location",
        programdata, exe_sibling
    ))
}