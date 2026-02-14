#![allow(unused_imports)]
use chrono::Local;
use chrono::{Duration, NaiveDateTime};
use dioxus::{CapturedError, prelude::*};
use dioxus_fullstack::ByteStream;
use once_cell::sync::Lazy;
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;
use std::io::Read;
use std::process::Command;
use std::sync::mpsc;
use tar::Builder;

const SYNC_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("puppet_sync");

pub static DB: Lazy<Database> = Lazy::new(|| {
    Database::create("cn-db.redb").expect("Failed to create database")
});

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PuppetStatus {
    pub hostname: String,
    pub status: String,
    pub exit_code: i32,
    pub timestamp: String,
    #[serde(default)]
    pub logs: String,
    #[serde(default)]
    pub checkin_logs: Vec<String>,
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
    logs: String,
    checkin_logs: Vec<String>,
) -> Result<(), ServerFnError> {
    const MAX_KEY_SIZE: usize = 1_048_576;
    const MAX_KEYS: usize = 500;
    const KEYS_TO_REMOVE: usize = 100;

    let now = Local::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();

    let puppet_status = PuppetStatus {
        hostname: hostname.clone(),
        status: status.clone(),
        exit_code,
        timestamp: timestamp.clone(),
        logs: logs.clone(),
        checkin_logs: checkin_logs.clone(),
    };

    let json =
        serde_json::to_string(&puppet_status).map_err(|e| ServerFnError::new(e.to_string()))?;

    if json.len() > MAX_KEY_SIZE {
        return Err(ServerFnError::new(format!(
            "Data too large: {} bytes exceeds {} byte limit",
            json.len(),
            MAX_KEY_SIZE
        )));
    }

    let write_txn = DB
        .begin_write()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    {
        let table = write_txn
            .open_table(SYNC_TABLE)
            .map_err(|e| ServerFnError::new(e.to_string()))?;

        let key_count = table.len().map_err(|e| ServerFnError::new(e.to_string()))?;

        if key_count >= MAX_KEYS as u64 {
            let mut keys_to_delete = Vec::new();
            let mut iter = table
                .iter()
                .map_err(|e| ServerFnError::new(e.to_string()))?;

            for _ in 0..KEYS_TO_REMOVE.min(key_count as usize) {
                if let Some(item) = iter.next() {
                    let (key, _) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
                    keys_to_delete.push(key.value().to_string());
                }
            }
            drop(iter);
            drop(table);

            let mut table = write_txn
                .open_table(SYNC_TABLE)
                .map_err(|e| ServerFnError::new(e.to_string()))?;
            for key in &keys_to_delete {
                table
                    .remove(key.as_str())
                    .map_err(|e| ServerFnError::new(e.to_string()))?;
            }

            println!(
                "Storage limit reached ({} keys), removed {} oldest entries",
                key_count,
                keys_to_delete.len()
            );
        }
    }

    {
        let mut table = write_txn
            .open_table(SYNC_TABLE)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        let key = format!("sync:{}:{}", timestamp, hostname);
        table
            .insert(key.as_str(), json.as_bytes())
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    write_txn
        .commit()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    Ok(())
}

#[server]
pub async fn get_sync_table() -> Result<SyncTableData, ServerFnError> {
    #[derive(Debug, Clone)]
    struct SyncTableEntry {
        hostname: String,
        status: String,
        timestamp: String,
    }

    let mut all_syncs: Vec<SyncTableEntry> = Vec::new();
    let mut hostnames_set = std::collections::HashSet::new();

    let read_txn = DB
        .begin_read()
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let table = read_txn
        .open_table(SYNC_TABLE)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let iter = table
        .iter()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for item in iter {
        let (_key, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
        let value_bytes = value.value();
        let value_str = String::from_utf8_lossy(value_bytes);

        let sync: PuppetStatus = match serde_json::from_str(&value_str) {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to deserialize sync data: {}", e);
                continue;
            }
        };

        hostnames_set.insert(sync.hostname.clone());

        all_syncs.push(SyncTableEntry {
            hostname: sync.hostname,
            status: sync.status,
            timestamp: sync.timestamp,
        });
    }

    let mut interval_map: BTreeMap<(i64, String), SyncTableEntry> = BTreeMap::new();

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CheckinLogEntry {
    pub hostname: String,
    pub log: String,
}

#[server]
pub async fn get_all_checkin_logs() -> Result<Vec<CheckinLogEntry>, ServerFnError> {
    let mut checkin_entries: Vec<(String, CheckinLogEntry)> = Vec::new();

    let read_txn = DB
        .begin_read()
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let table = read_txn
        .open_table(SYNC_TABLE)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let iter = table
        .iter()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for item in iter {
        let (_, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
        let value_bytes = value.value();
        let value_str = String::from_utf8_lossy(value_bytes);

        let sync: PuppetStatus = match serde_json::from_str(&value_str) {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to deserialize sync data: {}", e);
                continue;
            }
        };

        for log in sync.checkin_logs {
            checkin_entries.push((
                sync.timestamp.clone(),
                CheckinLogEntry {
                    hostname: sync.hostname.clone(),
                    log,
                },
            ));
        }
    }

    checkin_entries.sort_by(|a, b| b.0.cmp(&a.0));

    Ok(checkin_entries.into_iter().map(|(_, entry)| entry).collect())
}

#[server]
pub async fn get_checkin_log(
    hostname: String,
    log_text: String,
) -> Result<Option<CheckinLogEntry>, ServerFnError> {
    let read_txn = DB
        .begin_read()
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let table = read_txn
        .open_table(SYNC_TABLE)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let iter = table
        .iter()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for item in iter {
        let (_, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
        let value_bytes = value.value();
        let value_str = String::from_utf8_lossy(value_bytes);

        let sync: PuppetStatus = match serde_json::from_str(&value_str) {
            Ok(s) => s,
            Err(_) => continue,
        };

        if sync.hostname == hostname {
            for log in &sync.checkin_logs {
                if log == &log_text {
                    return Ok(Some(CheckinLogEntry {
                        hostname: sync.hostname.clone(),
                        log: log.clone(),
                    }));
                }
            }
        }
    }

    Ok(None)
}

#[server]
pub async fn get_logs_for_interval(
    time: String,
    hostname: String,
) -> Result<Vec<PuppetStatus>, ServerFnError> {
    let mut interval_syncs: Vec<PuppetStatus> = Vec::new();

    let read_txn = DB
        .begin_read()
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let table = read_txn
        .open_table(SYNC_TABLE)
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    let iter = table
        .iter()
        .map_err(|e| ServerFnError::new(e.to_string()))?;

    for item in iter {
        let (_, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
        let value_bytes = value.value();
        let value_str = String::from_utf8_lossy(value_bytes);

        let sync: PuppetStatus = match serde_json::from_str(&value_str) {
            Ok(s) => s,
            Err(e) => {
                println!("Failed to deserialize sync data: {}", e);
                continue;
            }
        };

        if sync.hostname != hostname {
            continue;
        }

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

#[get("/manifests")]
pub async fn get_manifests() -> Result<ByteStream, ServerFnError> {
    let (send, recv) = mpsc::sync_channel::<Vec<u8>>(8);

    std::thread::spawn(move || {
        struct ChannelWriter {
            sender: std::sync::mpsc::SyncSender<Vec<u8>>,
        }

        impl std::io::Write for ChannelWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                self.sender.send(buf.to_vec()).map_err(|_| {
                    std::io::Error::new(std::io::ErrorKind::BrokenPipe, "Channel closed")
                })?;
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

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