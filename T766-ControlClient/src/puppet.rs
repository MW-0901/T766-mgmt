use crate::client::Client;
use crate::host::{hostname, os};
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
    logs: String,
    checkin_logs: Vec<String>
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
        let dir_name = dir.path().to_str().unwrap();
        info!("Applying manifests...");
        let result = self.apply_dir(dir_name);
        info!("Returning status to server...");
        self.client.send_status(result)
    }

    fn build_puppet_command(dir_name: &str) -> Command {
        let module_path = format!("{}/modules", dir_name);

        let mut command = if os() == "windows" {
            let mut cmd = Command::new("cmd");
            cmd.args([
                "/C", "puppet", "apply",
                "--color=false",
                "--modulepath", &module_path,
                dir_name
            ]);
            cmd
        } else {
            let mut cmd = Command::new("puppet");
            cmd.args([
                "apply",
                "--color=false",
                "--modulepath", &module_path,
                dir_name
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

    fn apply_dir(&self, dir_name: &str) -> ApplyResult {
        let manifest_path = format!("{}/manifests", dir_name);
        println!("{manifest_path}");
        let result = Self::build_puppet_command(manifest_path.as_str())
            .output()
            .unwrap_or_else(|_| panic!("Failed to run puppet apply {}", dir_name));

        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr),
        );

        let exit_code = result.status.code().unwrap_or(-1);
        let checkin_logs = match checkin_logs() {
            Ok(logs) | Err(logs) => logs,
        };

        let apply_result = ApplyResult {
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
            checkin_logs,
        };

        if let Err(e) = clear_logs() {
            warn!("Failed to clear checkin logs: {}", e);
        }

        apply_result
    }
}