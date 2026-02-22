#![cfg_attr(all(windows, not(feature = "run")), windows_subsystem = "windows")]
use crate::puppet::PuppetClient;

mod client;
mod host;
mod puppet;
mod config;

use chrono::{Local, DateTime, Timelike, Duration};
use std::{thread, fs, path::PathBuf, sync::Arc, sync::atomic::{AtomicBool, Ordering}};
use std::process::exit;
use log::{info, error, warn};
use config::log_path;

const CATCH_UP_WINDOW_MINUTES: i64 = 15;
const MAX_CONSECUTIVE_FAILURES: u32 = 5;
const MIN_BACKOFF_SECONDS: u64 = 30;
const MAX_BACKOFF_SECONDS: u64 = 300;

fn get_state_file() -> Result<PathBuf, String> {
    let local_appdata = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .unwrap_or_else(|_| ".".to_string());

    let state_dir = PathBuf::from(local_appdata).join("T766 Control System");
    fs::create_dir_all(&state_dir)
        .map_err(|e| format!("Failed to create state directory: {}", e))?;
    Ok(state_dir.join("last_run.txt"))
}

fn load_last_run() -> Option<DateTime<Local>> {
    let path = get_state_file().ok()?;
    let content = fs::read_to_string(&path).ok()?;
    let timestamp = content.trim().parse::<i64>().ok()?;
    let dt = DateTime::from_timestamp(timestamp, 0)?;
    let local_dt = dt.with_timezone(&Local);

    let now = Local::now();
    if local_dt > now || (now - local_dt).num_days() > 7 {
        return None;
    }

    Some(local_dt)
}

fn save_last_run(time: DateTime<Local>) -> Result<(), String> {
    let path = get_state_file()?;
    let temp_path = path.with_extension("tmp");

    fs::write(&temp_path, time.timestamp().to_string())
        .map_err(|e| format!("Failed to write temp state file: {}", e))?;
    fs::rename(&temp_path, &path)
        .map_err(|e| format!("Failed to rename temp state file: {}", e))?;

    Ok(())
}

fn get_last_scheduled_run() -> DateTime<Local> {
    let now = Local::now();
    let mut target = now.with_second(0).unwrap().with_nanosecond(0).unwrap();
    let minute = now.minute();

    if minute < 30 {
        target = target.with_minute(0).unwrap();
    } else {
        target = target.with_minute(30).unwrap();
    }

    target
}

fn get_next_scheduled_run() -> DateTime<Local> {
    let now = Local::now();
    let mut target = now.with_second(0).unwrap().with_nanosecond(0).unwrap();
    let minute = now.minute();

    if minute < 30 {
        target = target.with_minute(30).unwrap();
    } else {
        target = target.with_minute(0).unwrap() + Duration::hours(1);
    }

    target
}

fn run_sync(client: &PuppetClient) -> Result<(), String> {
    let start_time = Local::now();
    info!("Running sync at {}...", start_time);

    let result = client.apply().map_err(|e| format!("Sync failed: {}", e))?;
    info!("Sync successful! Control node response:\n {}", result);

    let end_time = Local::now();
    info!("Finished sync at {}\n\n", end_time);

    save_last_run(end_time).ok();
    Ok(())
}

fn should_catch_up() -> bool {
    let last_scheduled = get_last_scheduled_run();
    let now = Local::now();

    if let Some(last_run) = load_last_run() {
        if last_run >= last_scheduled {
            return false;
        }
    }

    let time_since_scheduled = now - last_scheduled;
    let minutes_since = time_since_scheduled.num_minutes();

    if minutes_since < 0 {
        warn!("Clock appears to have gone backwards");
        return false;
    }

    if minutes_since <= CATCH_UP_WINDOW_MINUTES {
        info!("Missed run at {} ({}m ago) - catching up!", last_scheduled, minutes_since);
        true
    } else {
        warn!("Missed run at {} ({}m ago) - too late to catch up", last_scheduled, minutes_since);
        false
    }
}

fn wait_for_next_run(shutdown: &Arc<AtomicBool>) -> Result<(), String> {
    let now = Local::now();
    let target = get_next_scheduled_run();
    let duration = target - now;

    if duration < Duration::zero() {
        return Ok(());
    }

    let sleep_duration = duration.to_std()
        .map_err(|e| format!("Invalid sleep duration: {}", e))?;

    info!("Waiting until {} ({} seconds)", target, sleep_duration.as_secs());

    let check_interval = std::time::Duration::from_secs(10);
    let mut remaining = sleep_duration;

    while remaining > std::time::Duration::ZERO {
        if shutdown.load(Ordering::Relaxed) {
            return Err("Shutdown requested".to_string());
        }
        let sleep_time = remaining.min(check_interval);
        thread::sleep(sleep_time);
        remaining = remaining.saturating_sub(sleep_time);
    }

    Ok(())
}

fn calculate_backoff(failure_count: u32) -> std::time::Duration {
    let backoff_secs = MIN_BACKOFF_SECONDS * 2u64.pow(failure_count.min(5));
    let backoff_secs = backoff_secs.min(MAX_BACKOFF_SECONDS);
    std::time::Duration::from_secs(backoff_secs)
}

fn rotate_log_if_needed(log_path: &PathBuf) {
    const MAX_LOG_SIZE: u64 = 5 * 1024 * 1024; // 5MB
    if let Ok(metadata) = fs::metadata(log_path) {
        if metadata.len() > MAX_LOG_SIZE {
            let old = log_path.with_extension("old.log");
            let _ = fs::rename(log_path, old);
        }
    }
}

fn setup_logging() -> Result<(), fern::InitError> {
    rotate_log_if_needed(&log_path());
    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stderr())
        .chain(fern::log_file(log_path())?)
        .apply()?;
    Ok(())
}

fn main() {
    setup_logging().expect("Failed to initialize logging");

    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    ctrlc::set_handler(move || {
        shutdown_clone.store(true, Ordering::Relaxed);
    }).ok();

    if cfg!(feature = "run") {
        let client = PuppetClient::new();
        match run_sync(&client) {
            Ok(_) => exit(0),
            Err(e) => {
                error!("Manual sync failed: {}", e);
                exit(1);
            }
        }
    }

    info!("T766 Control Client starting...");
    let client = PuppetClient::new();
    let mut consecutive_failures: u32 = 0;

    if should_catch_up() {
        match run_sync(&client) {
            Ok(_) => consecutive_failures = 0,
            Err(e) => {
                error!("Catch-up sync failed: {}", e);
                consecutive_failures += 1;
            }
        }
    }

    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match wait_for_next_run(&shutdown) {
            Ok(_) => {}
            Err(_) => break,
        }

        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match run_sync(&client) {
            Ok(_) => {
                consecutive_failures = 0;
            }
            Err(e) => {
                error!("Sync failed: {}", e);
                consecutive_failures += 1;

                if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                    let backoff = calculate_backoff(consecutive_failures - MAX_CONSECUTIVE_FAILURES);
                    warn!("Backing off for {} seconds", backoff.as_secs());

                    let check_interval = std::time::Duration::from_secs(5);
                    let mut remaining = backoff;

                    while remaining > std::time::Duration::ZERO {
                        if shutdown.load(Ordering::Relaxed) {
                            break;
                        }
                        let sleep_time = remaining.min(check_interval);
                        thread::sleep(sleep_time);
                        remaining = remaining.saturating_sub(sleep_time);
                    }
                }
            }
        }
    }

    info!("T766 Control Client stopped");
}