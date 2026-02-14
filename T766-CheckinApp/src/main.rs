#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::error::Error;
use rdev::display_size;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use chrono::Local;

slint::include_modules!();

fn submit_id(ui: &AppWindow, id: String) -> bool {
    println!("Submitted: {}", id);
    let mut valid = true;
    for c in id.chars() {
        if !c.is_digit(10) {
            valid = false;
        }
    }
    if id.len() != 6 {
        valid = false;
    }
    if !valid {
        ui.invoke_show_error();
        return false
    }

    let path = if cfg!(windows) {
        let user = std::env::var("USERNAME").unwrap_or_else(|_| "Default".to_string());
        PathBuf::from(format!(r"C:\Users\{}\AppData\Local\T766 Control System\checkin-logs", user))
    } else {
        PathBuf::from("/etc/t766/checkin-logs")
    };

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("{} - {}\n\n\n", timestamp, id);

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = file.write_all(entry.as_bytes());
    }
    true
}
fn main() -> Result<(), Box<dyn Error>> {
    std::env::set_var("SLINT_BACKEND", "winit-femtovg");
    let ui = AppWindow::new()?;

    let (screen_w, screen_h) = display_size().expect("Failed to get display size");
    let target_width = 300;
    let target_height = 300;
    let scale_x = screen_w as f64 / target_width as f64;
    let scale_y = screen_h as f64 / target_height as f64;
    let scale_factor = scale_x.min(scale_y);
    std::env::set_var("SLINT_SCALE_FACTOR", scale_factor.to_string());

    ui.on_submit_id({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.unwrap();
            let id = ui.get_id().to_string();
            if submit_id(&ui, id) {
                ui.window().hide().unwrap();
            }
        }
    });

    ui.run()?;

    Ok(())
}
