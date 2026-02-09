use std::io::Cursor;
use tar::Archive;
use tempfile::TempDir;
use serde_json;
use log::error;
use crate::puppet::ApplyResult;

static URL_ONE: &'static str = "http://100.82.13.20:5000"; // Will be the local pi
static URL_TWO: &'static str = "http://100.82.13.20:5000"; // Will be the VPS

pub struct Client {}
impl Client {
    pub fn new() -> Self {
        Client {}
    }

    fn req_manifests(&self) -> Result<Vec<u8>, String> {
        let url_one = format!("{}/manifests", URL_ONE);
        let url_two = format!("{}/manifests", URL_TWO);
        match minreq::get(&url_one)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .send()
        {
            Ok(response) => return Ok(response.into_bytes()),
            Err(err) => {
                error!("Local control node connection failed: {}", err);
            }
        }

        let response = minreq::get(&url_two)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .send();

        match response {
            Ok(response) => Ok(response.into_bytes()),
            Err(err) => {
                error!("VPS connection failed: {}", err);
                Err(err.to_string())
            }
        }
    }

    pub fn manifests(&self) -> Result<TempDir, String> {
        let tarball = self.req_manifests()?;

        let temp_dir = TempDir::new().map_err(|e| e.to_string())?;

        let cursor = Cursor::new(tarball);
        let mut archive = Archive::new(cursor);

        archive.unpack(temp_dir.path()).map_err(|e| e.to_string())?;

        Ok(temp_dir)
    }

    pub fn send_status(&self, status: ApplyResult) -> Result<String, String> {
        let url_one = format!("{}/puppet-sync", URL_ONE);
        let url_two = format!("{}/puppet-sync", URL_TWO);
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
