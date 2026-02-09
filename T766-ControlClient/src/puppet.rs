use crate::client::Client;
use crate::host::{hostname, os};
use std::process::Command;
use serde::Serialize;

pub struct PuppetClient {
    client: Client,
}

#[derive(Debug, Serialize)]
pub struct ApplyResult {
    hostname: String,
    status: String,
    exit_code: i32,
    logs: String,
}

impl PuppetClient {
    pub fn new() -> Self {
        PuppetClient {
            client: Client::new(),
        }
    }

    pub fn apply(&self) -> Result<String, String> {
        let dir = self.client.manifests()?;
        let dir_name = dir.path().to_str().unwrap();
        let result = self.apply_dir(dir_name);
        let resp = self.client.send_status(result)?;
        Ok(resp)
    }

    fn apply_dir(&self, dir_name: &str) -> ApplyResult {
        let cmd: &str;
        if os() == "windows" {
            cmd = "puppet.bat";
        } else {
            cmd = "puppet";
        }
        let result = Command::new(cmd)
            .arg("apply")
            .arg("--color=false")
            .arg(dir_name)
            .output()
            .expect(format!("Failed to run: 'puppet apply {}'", dir_name).as_str());
        ApplyResult {
            hostname: hostname(),
            status: if result.status.success() {
                "success".to_string()
            } else {
                "failure".to_string()
            },
            exit_code: result.status.code().unwrap_or(-1),
            logs: String::from_utf8_lossy(&result.stdout).to_string(),
        }
    }
}
