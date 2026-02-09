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
        let (cmd, args): (&str, Vec<&str>) = if os() == "windows" {
            (
                "cmd",
                vec![
                    "/C",
                    "puppet",
                    "apply",
                    "--color=false",
                    dir_name,
                ],
            )
        } else {
            (
                "puppet",
                vec![
                    "apply",
                    "--color=false",
                    dir_name,
                ],
            )
        };

        let mut command = Command::new(cmd);
        command.args(&args);

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            use winapi::um::winbase::{
                CREATE_NEW_PROCESS_GROUP,
                CREATE_NO_WINDOW,
            };

            command.creation_flags(
                CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW,
            );
        }

        let result = command
            .output()
            .expect(format!("Failed to run puppet apply {}", dir_name).as_str());

        let logs = format!(
            "{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr),
        );

        let exit_code = result.status.code().unwrap_or(-1);

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
        }
    }
}
