/// Puppet sync data types, ingestion, and query logic.
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct SyncTableData {
    pub times: Vec<String>,
    pub hostnames: Vec<String>,
    pub syncs: HashMap<String, HashMap<String, String>>,
}

#[cfg(feature = "server")]
use chrono::{Local, NaiveDateTime};
#[cfg(feature = "server")]
use std::collections::{BTreeMap, HashSet};

/// Compute the 15-minute interval index for a timestamp string (YYYYMMDDHHmmSS).
#[cfg(feature = "server")]
fn interval_index(timestamp: &str) -> Option<i64> {
    let dt = NaiveDateTime::parse_from_str(timestamp, "%Y%m%d%H%M%S").ok()?;
    Some(dt.and_utc().timestamp() / 60 / 15)
}

/// Format an interval index as a display string.
#[cfg(feature = "server")]
fn format_interval(idx: i64) -> String {
    chrono::DateTime::from_timestamp(idx * 15 * 60, 0)
        .unwrap()
        .with_timezone(&Local)
        .format("%-I:%M %p %-m-%-d-%y")
        .to_string()
}

/// Ingest a puppet sync report.
#[post("/puppet-sync")]
pub async fn handle_sync(
    hostname: String,
    status: String,
    exit_code: i32,
    logs: String,
    checkin_logs: Vec<String>,
) -> Result<(), ServerFnError> {
    let now = Local::now();
    let timestamp = now.format("%Y%m%d%H%M%S").to_string();

    let record = PuppetStatus {
        hostname: hostname.clone(),
        status, exit_code,
        timestamp: timestamp.clone(),
        logs, checkin_logs,
    };

    let json = serde_json::to_string(&record).map_err(|e| ServerFnError::new(e.to_string()))?;
    let key = format!("sync:{}:{}", timestamp, hostname);
    crate::db::insert_sync(&key, &json)
}

/// Build the sync overview table (last 20 intervals x hostnames).
#[server]
pub async fn get_sync_table() -> Result<SyncTableData, ServerFnError> {
    let all = crate::db::iter_all_syncs()?;

    let mut hostnames_set = HashSet::new();
    let mut by_interval: BTreeMap<(i64, String), (String, String)> = BTreeMap::new();

    for s in &all {
        hostnames_set.insert(s.hostname.clone());
        let Some(idx) = interval_index(&s.timestamp) else { continue };
        let key = (idx, s.hostname.clone());
        match by_interval.get(&key) {
            Some((ts, _)) if *ts > s.timestamp => {}
            _ => { by_interval.insert(key, (s.timestamp.clone(), s.status.clone())); }
        }
    }

    let mut interval_indices: Vec<i64> = by_interval.keys().map(|(idx, _)| *idx).collect();
    interval_indices.dedup();
    interval_indices.reverse();
    interval_indices.truncate(20);

    let times: Vec<String> = interval_indices.iter().map(|idx| format_interval(*idx)).collect();
    let mut syncs = HashMap::new();
    for idx in &interval_indices {
        let display = format_interval(*idx);
        let hosts: HashMap<String, String> = by_interval.iter()
            .filter(|((i, _), _)| i == idx)
            .map(|((_, h), (_, st))| (h.clone(), st.clone()))
            .collect();
        syncs.insert(display, hosts);
    }

    let mut hostnames: Vec<String> = hostnames_set.into_iter().collect();
    hostnames.sort();

    Ok(SyncTableData { times, hostnames, syncs })
}

/// Get all sync records for a specific hostname within a display interval.
#[server]
pub async fn get_logs_for_interval(
    time: String,
    hostname: String,
) -> Result<Vec<PuppetStatus>, ServerFnError> {
    let all = crate::db::iter_all_syncs()?;

    let mut results: Vec<PuppetStatus> = all.into_iter()
        .filter(|s| {
            s.hostname == hostname
                && interval_index(&s.timestamp)
                    .map(|idx| format_interval(idx) == time)
                    .unwrap_or(false)
        })
        .collect();

    results.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
    Ok(results)
}
