use crate::client::Client;
use crate::host::{hostname, os};
use std::path::Path;
use std::process::Command;
use log::{info, warn};
use serde::Serialize;
use crate::config::{checkin_logs, clear_logs};

pub struct PuppetClient {
    client: Client,
}

#[derive(Debug, Serialize)]
pub struct ApplyResult {
    hostname: String,
    status: String,
    exit_code: i32,
    pub logs: String,
    pub checkin_logs: Vec<String>
}

impl PuppetClient {
    pub fn new() -> Self {
        PuppetClient {
            client: Client::new(),
        }
    }

    pub fn apply(&self) -> Result<String, String> {
        info!("Fetching manifests...");
        let dir = self.client.manifests()?;

        info!("Applying manifests...");
        let manifest_path = dir.path().join("manifests");
        let result = self.apply_dir(&manifest_path);

        info!("Returning status to server...");
        self.client.send_status(result)
    }

    fn build_puppet_command(manifest_dir: &Path) -> Command {
        let module_path = manifest_dir.parent()
            .map(|p| p.join("modules"))
            .unwrap_or_else(|| manifest_dir.join("modules"));

        let manifest_str = manifest_dir.to_string_lossy();
        let module_str = module_path.to_string_lossy();

        let command = if os() == "windows" {
            let mut cmd = Command::new("cmd");
            cmd.args([
                "/C", "puppet", "apply",
                "--color=false",
                "--modulepath", &module_str,
                &manifest_str,
            ]);
            cmd
        } else {
            let mut cmd = Command::new("puppet");
            cmd.args([
                "apply",
                "--color=false",
                "--modulepath", &module_str,
                &manifest_str,
            ]);
            cmd
        };

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use winapi::um::winbase::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};
            command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
        }

        command
    }

    fn apply_dir(&self, manifest_dir: &Path) -> ApplyResult {
        let result = match Self::build_puppet_command(manifest_dir).output() {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to run puppet apply {:?}: {}", manifest_dir, e);
                return ApplyResult {
                    hostname: hostname(),
                    status: "failure".to_string(),
                    exit_code: -1,
                    logs: format!("Failed to run puppet apply: {}", e),
                    checkin_logs: Vec::new(),
                };
            }
        };

        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr),
        );

        let exit_code = result.status.code().unwrap_or(-1);
        let checkin = match checkin_logs() {
            Ok(logs) => logs,
            Err(e) => {
                warn!("Failed to read checkin logs: {:?}", e);
                Vec::new()
            }
        };

        if let Err(e) = clear_logs() {
            warn!("Failed to clear checkin logs: {}", e);
        }

        ApplyResult {
            hostname: hostname(),
            status: if result.status.success() {
                "success".to_string()
            } else if exit_code == -1 {
                "interrupted".to_string()
            } else {
                "failure".to_string()
            },
            exit_code,
            logs,
            checkin_logs: checkin,
        }
    }
}
