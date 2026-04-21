/// Checkin log queries — extracts checkin data from puppet sync records.
use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct CheckinLogEntry {
    pub hostname: String,
    pub log: String,
}

#[cfg(feature = "server")]
use once_cell::sync::Lazy;
#[cfg(feature = "server")]
use regex::Regex;
#[cfg(feature = "server")]
use std::collections::HashMap;
#[cfg(feature = "server")]
use std::io::BufReader;

#[cfg(feature = "server")]
static CODE_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"\b(\d{6})\b").unwrap());

/// Load the code-to-name mapping from CSV.
#[cfg(feature = "server")]
fn load_people_map() -> HashMap<String, String> {
    let file = match std::fs::File::open("/opt/puppet/people.csv") {
        Ok(f) => f,
        Err(_) => return HashMap::new(),
    };

    let mut reader = csv::Reader::from_reader(BufReader::new(file));
    let mut map = HashMap::new();
    for record in reader.records().flatten() {
        if let (Some(code), Some(name)) = (record.get(0), record.get(1)) {
            if !code.is_empty() && !name.is_empty() {
                map.insert(code.to_string(), name.to_string());
            }
        }
    }
    map
}

/// Replace 6-digit codes with names from the people map.
#[cfg(feature = "server")]
fn replace_codes(text: &str, people: &HashMap<String, String>) -> String {
    CODE_REGEX.replace_all(text, |caps: &regex::Captures| {
        let code = caps.get(1).unwrap().as_str();
        people.get(code).cloned().unwrap_or_else(|| code.to_string())
    }).to_string()
}

/// Get all checkin logs across all sync records, sorted newest first.
#[server]
pub async fn get_all_checkin_logs() -> Result<Vec<CheckinLogEntry>, ServerFnError> {
    let people = load_people_map();
    let all = crate::db::iter_all_syncs()?;

    let mut entries: Vec<(String, CheckinLogEntry)> = Vec::new();
    for s in all {
        for log in s.checkin_logs {
            entries.push((s.timestamp.clone(), CheckinLogEntry {
                hostname: s.hostname.clone(),
                log: replace_codes(&log, &people),
            }));
        }
    }

    entries.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(entries.into_iter().map(|(_, e)| e).collect())
}

/// Find a specific checkin log by hostname and processed text.
#[server]
pub async fn get_checkin_log(
    hostname: String,
    log_text: String,
) -> Result<Option<CheckinLogEntry>, ServerFnError> {
    let people = load_people_map();
    let all = crate::db::iter_all_syncs()?;

    for s in all {
        if s.hostname != hostname { continue; }
        for log in &s.checkin_logs {
            let processed = replace_codes(log, &people);
            if processed == log_text {
                return Ok(Some(CheckinLogEntry {
                    hostname: s.hostname.clone(),
                    log: processed,
                }));
            }
        }
    }
    Ok(None)
}
