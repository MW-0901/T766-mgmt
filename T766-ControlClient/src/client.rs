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

pub struct Client;

impl Client {
    pub fn new() -> Self {
        Client
    }

    /// Try a request against the primary URL, then fall back to the fallback URL.
    fn request_with_fallback<F, T>(&self, make_request: F) -> Result<T, String>
    where
        F: Fn(&str) -> Result<T, String>,
    {
        match make_request(&CONFIG.primary_url) {
            Ok(v) => return Ok(v),
            Err(e) => error!("Primary connection failed: {}", e),
        }
        info!("Falling back to remote control node...");
        make_request(&CONFIG.fallback_url)
            .map_err(|e| { error!("Fallback connection failed: {}", e); e })
    }

    fn http_get(url: &str) -> Result<Vec<u8>, String> {
        let response = minreq::get(url)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .send()
            .map_err(|e| e.to_string())?;
        if response.status_code != 200 {
            return Err(format!("status code {}", response.status_code));
        }
        Ok(response.into_bytes())
    }

    fn http_post(url: &str, body: &str) -> Result<String, String> {
        let response = minreq::post(url)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .with_body(body)
            .send()
            .map_err(|e| e.to_string())?;
        Ok(String::from_utf8_lossy(&response.into_bytes()).into_owned())
    }

    pub fn manifests(&self) -> Result<TempDir, String> {
        let tarball = self.request_with_fallback(|base| {
            let url = format!("{}manifests", base);
            info!("{}", url);
            Self::http_get(&url)
        })?;

        let temp_dir = TempDir::new().map_err(|e| e.to_string())?;
        let mut archive = Archive::new(Cursor::new(&tarball));
        archive.unpack(temp_dir.path()).map_err(|e| {
            format!("Error opening tarball: {e}\nServer response: {:?}",
                    String::from_utf8_lossy(&tarball))
        })?;
        Ok(temp_dir)
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

        let body = serde_json::to_string(&status).map_err(|e| e.to_string())?;

        self.request_with_fallback(|base| {
            let url = format!("{}puppet-sync", base);
            Self::http_post(&url, &body)
        })
    }
}
