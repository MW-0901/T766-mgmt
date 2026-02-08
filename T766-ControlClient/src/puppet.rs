use crate::client::Client;
use crate::host::{hostname, os};
use std::process::Command;

pub struct PuppetClient {
    client: Client,
}

#[derive(Debug)]
pub struct ApplyResult {
    hostname: String,
    status: String,
    exit_code: String,
    logs: String,
}

impl PuppetClient {
    pub fn new() -> Self {
        PuppetClient {
            client: Client::new(),
        }
    }

    pub fn apply(&self) -> Result<ApplyResult, String> {
        let dir = self.client.manifests()?;
        let dir_name = dir.path().to_str().unwrap();
        Ok(self.apply_dir(dir_name))
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
            exit_code: result.status.to_string(),
            logs: String::from_utf8_lossy(&result.stdout).to_string(),
        }
    }
}
