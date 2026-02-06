#![allow(unused_imports)]
use chrono::Local;
use chrono::{Duration, NaiveDateTime};
use dioxus::{CapturedError, prelude::*};
use dioxus_fullstack::ByteStream;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sled::Db;
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::io::Read;
use std::process::Command;
use std::sync::mpsc;
use tar::Builder;

#[allow(dead_code)]
pub static DB: Lazy<Db> = Lazy::new(|| sled::open("cn-db").expect("Failed to open database"));

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PuppetStatus {
    pub hostname: String,
    pub status: String,
    pub exit_code: i32,
    pub timestamp: String,
    #[serde(default)]
    pub display_timestamp: String,
    #[serde(default)]
    pub manifests_applied: Vec<String>,
    #[serde(default)]
    pub manifests_failed: Vec<String>,
    #[serde(default)]
    pub total_manifests: i32,
    #[serde(default)]
    pub logs: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SyncTableData {
    pub times: Vec<String>,
    pub hostnames: Vec<String>,
    pub syncs: HashMap<String, HashMap<String, String>>,
}

#[post("/puppet-sync")]
pub async fn handle_sync(
    hostname: String,
    status: String,
    exit_code: i32,
    manifests_applied: Option<Vec<String>>,
    manifests_failed: Option<Vec<String>>,
    total_manifests: Option<i32>,
    logs: String,
) -> Result<(), ServerFnError> {
    let now = Local::now();
    let sortable_timestamp = now.format("%Y%m%d%H%M%S").to_string();
    let display_timestamp = now.format("%-I:%M %p %-m-%-d-%y").to_string();

    let puppet_status = PuppetStatus {
        hostname: hostname.clone(),
        status: status.clone(),
        exit_code,
        timestamp: sortable_timestamp.clone(),
        display_timestamp: display_timestamp.clone(),
        manifests_applied: manifests_applied.unwrap_or_default(),
        manifests_failed: manifests_failed.unwrap_or_default(),
        total_manifests: total_manifests.unwrap_or(0),
        logs: logs.clone(),
    };

    let json =
        serde_json::to_string(&puppet_status).map_err(|e| ServerFnError::new(e.to_string()))?;

    let key = format!("sync:{}:{}", sortable_timestamp, hostname);
    DB.insert(key.as_bytes(), json.as_bytes())
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    if !puppet_status.manifests_applied.is_empty() {
        println!("Applied Manifests: {:?}", puppet_status.manifests_applied);
    }

    if !puppet_status.manifests_failed.is_empty() {
        println!("Failed Manifests: {:?}", puppet_status.manifests_failed);
    }

    Ok(())
}

#[server]
pub async fn get_sync_table() -> Result<SyncTableData, ServerFnError> {
    let mut all_syncs: Vec<PuppetStatus> = Vec::new();
    let mut hostnames_set = std::collections::HashSet::new();

    for item in DB.scan_prefix(b"sync:") {
        let (key, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;

        let value_str = String::from_utf8_lossy(&value);
        let mut sync: PuppetStatus = match serde_json::from_str(&value_str) {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to deserialize sync data: {}", e);
                continue;
            }
        };

        if sync.display_timestamp.is_empty() {
            sync.display_timestamp = sync.timestamp.clone();
        }

        hostnames_set.insert(sync.hostname.clone());
        all_syncs.push(sync);
    }

    let mut interval_map: BTreeMap<(i64, String), PuppetStatus> = BTreeMap::new();

    for sync in all_syncs {
        let dt = match NaiveDateTime::parse_from_str(&sync.timestamp, "%Y%m%d%H%M%S") {
            Ok(d) => d,
            Err(_) => continue,
        };

        let minutes_since_epoch = dt.and_utc().timestamp() / 60;
        let interval_index = minutes_since_epoch / 15;

        let key = (interval_index, sync.hostname.clone());

        match interval_map.get(&key) {
            Some(existing) if existing.timestamp > sync.timestamp => {}
            _ => {
                interval_map.insert(key, sync);
            }
        }
    }

    let mut times_map: BTreeMap<i64, (String, HashMap<String, String>)> = BTreeMap::new();

    for ((interval_index, hostname), sync) in interval_map.iter() {
        let entry = times_map.entry(*interval_index).or_insert_with(|| {
            let interval_start =
                Local::now().with_timezone(&chrono::Utc).timestamp() / 60 / 15 * 15 * 60;
            let dt = chrono::DateTime::from_timestamp(interval_index * 15 * 60, 0)
                .unwrap_or_else(|| chrono::DateTime::from_timestamp(interval_start, 0).unwrap())
                .with_timezone(&Local);
            let display = dt.format("%-I:%M %p %-m-%-d-%y").to_string();
            (display, HashMap::new())
        });
        entry.1.insert(hostname.clone(), sync.status.clone());
    }

    let times: Vec<String> = times_map
        .iter()
        .rev()
        .take(20)
        .map(|(_, (display, _))| display.clone())
        .collect();

    let mut hostnames: Vec<String> = hostnames_set.into_iter().collect();
    hostnames.sort();

    let mut syncs = HashMap::new();
    for (_, (display, hosts)) in times_map.iter().rev().take(20) {
        syncs.insert(display.clone(), hosts.clone());
    }

    Ok(SyncTableData {
        times,
        hostnames,
        syncs,
    })
}

#[server]
pub async fn tailscale_status() -> Result<String, ServerFnError> {
    let output = Command::new("tailscale")
        .arg("status")
        .output()
        .expect("Failed to get tailscale status");
    Ok(String::from_utf8(output.stdout).unwrap())
}

#[server]
pub async fn get_logs_for_interval(
    time: String,
    hostname: String,
) -> Result<Vec<PuppetStatus>, ServerFnError> {
    let mut all_syncs: Vec<PuppetStatus> = Vec::new();

    for item in DB.scan_prefix(b"sync:") {
        let (_, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;

        let value_str = String::from_utf8_lossy(&value);
        let mut sync: PuppetStatus = match serde_json::from_str(&value_str) {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to deserialize sync data: {}", e);
                continue;
            }
        };

        if sync.display_timestamp.is_empty() {
            sync.display_timestamp = sync.timestamp.clone();
        }

        if sync.hostname == hostname {
            all_syncs.push(sync);
        }
    }

    let mut interval_syncs: Vec<PuppetStatus> = Vec::new();

    for sync in all_syncs {
        let dt = match NaiveDateTime::parse_from_str(&sync.timestamp, "%Y%m%d%H%M%S") {
            Ok(d) => d,
            Err(_) => continue,
        };

        let minutes_since_epoch = dt.and_utc().timestamp() / 60;
        let interval_index = minutes_since_epoch / 15;

        let dt_local = chrono::DateTime::from_timestamp(interval_index * 15 * 60, 0)
            .unwrap()
            .with_timezone(&Local);
        let display = dt_local.format("%-I:%M %p %-m-%-d-%y").to_string();

        if display == time {
            interval_syncs.push(sync);
        }
    }

    interval_syncs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(interval_syncs)
}

struct ChannelWriter {
    sender: std::sync::mpsc::SyncSender<Vec<u8>>,
}

impl std::io::Write for ChannelWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.sender
            .send(buf.to_vec())
            .map_err(|_| std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed"))?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[get("/manifests")]
pub async fn get_manifests() -> Result<ByteStream, ServerFnError> {
    let (send, recv) = mpsc::sync_channel::<Vec<u8>>(8);

    std::thread::spawn(move || {
        let writer = ChannelWriter { sender: send };
        let mut archive = Builder::new(writer);

        if let Err(e) = archive.append_dir_all("manifests", "/puppet/manifests") {
            eprintln!("Error adding manifests to archive: {}", e);
            return;
        }

        if let Err(e) = archive.finish() {
            eprintln!("Error finishing archive: {}", e);
        }
    });

    Ok(ByteStream::spawn(move |tx| async move {
        while let Ok(chunk) = recv.recv() {
            if tx.unbounded_send(chunk.into()).is_err() {
                break;
            }
        }
    }))
}

#[get("/data/{filename}")]
pub async fn get_data_file(filename: String) -> Result<ByteStream, ServerFnError> {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(ServerFnError::new("Invalid filename"));
    }
    if filename.starts_with('.') {
        return Err(ServerFnError::new("Invalid filename"));
    }

    let file_path = format!("/puppet/{}", filename);

    let (send, recv) = mpsc::sync_channel::<Vec<u8>>(8);

    std::thread::spawn(move || {
        let mut file = match std::fs::File::open(&file_path) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("Failed to open file {}: {}", file_path, e);
                return;
            }
        };

        let mut buffer = vec![0u8; 8192];

        loop {
            match file.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if send.send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading file: {}", e);
                    break;
                }
            }
        }
    });

    Ok(ByteStream::spawn(move |tx| async move {
        while let Ok(chunk) = recv.recv() {
            if tx.unbounded_send(chunk.into()).is_err() {
                break;
            }
        }
    }))
}
