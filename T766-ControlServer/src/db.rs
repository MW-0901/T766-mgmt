/// Database access layer for the puppet sync store.
use once_cell::sync::Lazy;
use redb::{Database, ReadableDatabase, ReadableTable, ReadableTableMetadata, TableDefinition};
use dioxus::prelude::*;
use crate::sync::PuppetStatus;

pub const SYNC_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("puppet_sync");

pub static DB: Lazy<Database> = Lazy::new(|| {
    Database::create("cn-db.redb").expect("Failed to create database")
});

const MAX_KEY_SIZE: usize = 1_048_576;
const MAX_KEYS: usize = 500;
const KEYS_TO_REMOVE: usize = 100;

/// Iterate all PuppetStatus records in the database, newest last.
pub fn iter_all_syncs() -> Result<Vec<PuppetStatus>, ServerFnError> {
    let read_txn = DB.begin_read().map_err(|e| ServerFnError::new(e.to_string()))?;
    let table = read_txn.open_table(SYNC_TABLE).map_err(|e| ServerFnError::new(e.to_string()))?;
    let iter = table.iter().map_err(|e| ServerFnError::new(e.to_string()))?;

    let mut results = Vec::new();
    for item in iter {
        let (_, value) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
        match serde_json::from_slice::<PuppetStatus>(value.value()) {
            Ok(s) => results.push(s),
            Err(e) => println!("Failed to deserialize sync data: {}", e),
        }
    }
    Ok(results)
}

/// Insert a sync record, evicting oldest entries if storage limit reached.
pub fn insert_sync(key: &str, json: &str) -> Result<(), ServerFnError> {
    if json.len() > MAX_KEY_SIZE {
        return Err(ServerFnError::new(format!(
            "Data too large: {} bytes exceeds {} byte limit", json.len(), MAX_KEY_SIZE
        )));
    }

    let write_txn = DB.begin_write().map_err(|e| ServerFnError::new(e.to_string()))?;
    evict_if_full(&write_txn)?;

    {
        let mut table = write_txn.open_table(SYNC_TABLE)
            .map_err(|e| ServerFnError::new(e.to_string()))?;
        table.insert(key, json.as_bytes())
            .map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    write_txn.commit().map_err(|e| ServerFnError::new(e.to_string()))?;
    Ok(())
}

/// Remove oldest entries when the table exceeds MAX_KEYS.
fn evict_if_full(write_txn: &redb::WriteTransaction) -> Result<(), ServerFnError> {
    let table = write_txn.open_table(SYNC_TABLE)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    let key_count = table.len().map_err(|e| ServerFnError::new(e.to_string()))?;

    if key_count < MAX_KEYS as u64 {
        return Ok(());
    }

    let mut keys_to_delete = Vec::new();
    let mut iter = table.iter().map_err(|e| ServerFnError::new(e.to_string()))?;
    for _ in 0..KEYS_TO_REMOVE.min(key_count as usize) {
        if let Some(item) = iter.next() {
            let (key, _) = item.map_err(|e| ServerFnError::new(e.to_string()))?;
            keys_to_delete.push(key.value().to_string());
        }
    }
    drop(iter);
    drop(table);

    let mut table = write_txn.open_table(SYNC_TABLE)
        .map_err(|e| ServerFnError::new(e.to_string()))?;
    for key in &keys_to_delete {
        table.remove(key.as_str()).map_err(|e| ServerFnError::new(e.to_string()))?;
    }

    println!("Storage limit reached ({} keys), removed {} oldest entries",
             key_count, keys_to_delete.len());
    Ok(())
}
