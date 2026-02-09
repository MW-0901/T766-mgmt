#![windows_subsystem = "windows"]
use crate::puppet::PuppetClient;

mod client;
mod host;
mod puppet;
use chrono::{Local, Timelike};
use std::{thread};
use env_logger;
use log::{info, error};

fn run_sync(client: &PuppetClient) {
    info!("Running sync at {}...", Local::now());
    match client.apply() {
        Err(e) => {
            error!("Sync ERROR: {}", e)
        }
        Ok(resp) => {
            info!("Sync successful! Control node response:\n {}", resp)
        }
    }
    info!("Finished sync at {}\n\n", Local::now());
}

fn next_half_hour() {
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

    let sleep_duration = (target - now)
        .to_std()
        .expect("Time went backwards, wtf???");
    thread::sleep(sleep_duration);
}

fn main() {
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    let client = PuppetClient::new();
    loop {
        next_half_hour();
        run_sync(&client);
    }
}
