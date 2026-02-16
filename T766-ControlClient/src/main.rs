#![cfg_attr(all(windows, not(feature = "run")), windows_subsystem = "windows")]
use crate::puppet::PuppetClient;

mod client;
mod host;
mod puppet;
mod config;

use chrono::{Local, DateTime, Timelike};
use std::{thread, fs, path::PathBuf};
use std::process::exit;
use env_logger;
use log::{info, error, warn};

fn get_state_file() -> PathBuf {
    let local_appdata = std::env::var("LOCALAPPDATA")
        .unwrap_or_else(|_| std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string()));

    let state_dir = PathBuf::from(local_appdata).join("T766 Control System");
    fs::create_dir_all(&state_dir).ok();
    state_dir.join("last_run.txt")
}

fn load_last_run() -> Option<DateTime<Local>> {
    let path = get_state_file();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(timestamp) = content.trim().parse::<i64>() {
            return DateTime::from_timestamp(timestamp, 0)
                .map(|dt| dt.with_timezone(&Local));
        }
    }
    None
}

fn save_last_run(time: DateTime<Local>) {
    let path = get_state_file();
    if let Err(e) = fs::write(&path, time.timestamp().to_string()) {
        warn!("Failed to save last run time: {}", e);
    }
}

fn get_last_scheduled_run() -> DateTime<Local> {
    let now = Local::now();
    let mut target = now
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();

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
    let mut target = now
        .with_second(0).unwrap()
        .with_nanosecond(0).unwrap();

    let minute = now.minute();

    if minute < 30 {
        target = target.with_minute(30).unwrap();
    } else {
        target = target
            .with_minute(0).unwrap()
            + chrono::Duration::hours(1);
    }

    target
}

fn run_sync(client: &PuppetClient) {
    let start_time = Local::now();
    info!("Running sync at {}...", start_time);

    match client.apply() {
        Err(e) => {
            error!("Sync ERROR: {}", e)
        }
        Ok(resp) => {
            info!("Sync successful! Control node response:\n {}", resp)
        }
    }

    let end_time = Local::now();
    info!("Finished sync at {}\n\n", end_time);

    save_last_run(end_time);
}

fn should_catch_up() -> bool {
    let last_scheduled = get_last_scheduled_run();
    let now = Local::now();

    if let Some(last_run) = load_last_run() {
        if last_run >= last_scheduled {
            info!("Last run was at {}, after scheduled time {}", last_run, last_scheduled);
            return false;
        }
    }

    let time_since_scheduled = now - last_scheduled;
    let minutes_since = time_since_scheduled.num_minutes();

    if minutes_since <= 15 {
        info!("Missed run at {} ({}m ago) - catching up!", last_scheduled, minutes_since);
        return true;
    } else {
        warn!("Missed run at {} ({}m ago) - too late to catch up", last_scheduled, minutes_since);
        return false;
    }
}

fn wait_for_next_run() {
    let now = Local::now();
    let target = get_next_scheduled_run();

    let sleep_duration = (target - now)
        .to_std()
        .expect("Time went backwards");

    info!("Waiting until {} ({} seconds)", target, sleep_duration.as_secs());
    thread::sleep(sleep_duration);
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();

    if cfg!(feature = "run") {
        println!("Running manual sync...");
        run_sync(&PuppetClient::new());
        exit(0);
    }

    info!("T766 Control Client starting...");
    let client = PuppetClient::new();

    if should_catch_up() {
        info!("Running catch-up sync immediately...");
        run_sync(&client);
    }

    loop {
        wait_for_next_run();
        run_sync(&client);
    }
}