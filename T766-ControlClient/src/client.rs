use std::io::Cursor;
use tar::Archive;
use tempfile::TempDir;
use serde_json;
use log::{error, info};
use std::sync::LazyLock;
use crate::config::{load_config, ClientConfig};
use crate::puppet::ApplyResult;

const MAX_LOG_BYTES: usize = 50_000;

static CONFIG: LazyLock<ClientConfig> = LazyLock::new(|| {
    load_config().expect("failed to load config")
});

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Client {}
    }

    fn req_manifests(&self) -> Result<Vec<u8>, String> {
        let url_one = format!("{}manifests", CONFIG.primary_url);
        let url_two = format!("{}manifests", CONFIG.fallback_url);
        info!("{}", url_one);
        match minreq::get(&url_one)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .send()
        {
            Ok(response) => {
                if response.status_code != 200 {
                    error!("Non-200 status code returned: {}", response.status_code);
                    return Err(format!("status code {} found", response.status_code));
                }
                return Ok(response.into_bytes())
            },
            Err(err) => {
                error!("Local control node connection failed: {}", err);
            }
        }

        info!("Falling back to remote control node...");
        let response = minreq::get(&url_two)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .send();

        match response {
            Ok(response) => {
                if response.status_code != 200 {
                    error!("Non-200 status code returned: {}", response.status_code);
                    return Err(format!("status code {} found", response.status_code));
                }
                Ok(response.into_bytes())
            },
            Err(err) => {
                error!("VPS connection failed: {}", err);
                Err(err.to_string())
            }
        }
    }

    pub fn manifests(&self) -> Result<TempDir, String> {
        let tarball = self.req_manifests()?;
        let temp_dir = TempDir::new().map_err(|e| e.to_string())?;
        let cursor = Cursor::new(&tarball);
        let mut archive = Archive::new(cursor);
        match archive.unpack(temp_dir.path()).map_err(|e| e.to_string()) {
            Ok(_) => Ok(temp_dir),
            Err(err) => {
                Err(format!("Error opening tarball: {err}\nServer response: {:?}", String::from_utf8_lossy(&tarball)))
            }
        }
    }

    fn truncate_log(log: &str) -> String {
        if log.len() <= MAX_LOG_BYTES {
            return log.to_string();
        }
        let truncated = &log[log.len() - MAX_LOG_BYTES..];
        format!("[...truncated...]\n{}", truncated)
    }

    pub fn send_status(&self, mut status: ApplyResult) -> Result<String, String> {
        status.logs = Self::truncate_log(&status.logs);
        status.checkin_logs = status.checkin_logs
            .into_iter()
            .map(|l| Self::truncate_log(&l))
            .collect();

        let url_one = format!("{}puppet-sync", CONFIG.primary_url);
        let url_two = format!("{}puppet-sync", CONFIG.fallback_url);
        let body = serde_json::to_string(&status)
            .map_err(|e| e.to_string())?;
        match minreq::post(&url_one)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .with_body(body.clone())
            .send()
        {
            Ok(response) => return Ok(String::from_utf8_lossy(&response.into_bytes()).into_owned()),
            Err(err) => {
                error!("Local control node connection failed: {}", err);
            }
        }

        let response = minreq::post(&url_two)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .with_body(body)
            .send();

        match response {
            Ok(response) => Ok(String::from_utf8_lossy(&response.into_bytes()).into_owned()),
            Err(err) => {
                error!("VPS connection failed: {}", err);
                Err(err.to_string())
            }
        }
    }
}