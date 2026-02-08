use std::io::Cursor;
use tar::Archive;
use tempfile::TempDir;
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
                eprintln!("Local control node connection failed: {}", err);
            }
        }

        let response = minreq::get(&url_two)
            .with_header("Accept-Encoding", "identity")
            .with_timeout(20)
            .send();

        match response {
            Ok(response) => Ok(response.into_bytes()),
            Err(err) => {
                eprintln!("VPS connection failed: {}", err);
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

    pub fn send_status(&self, status: ApplyResult) {
        
    }
}
