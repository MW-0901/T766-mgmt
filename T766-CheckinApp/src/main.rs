#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::error::Error;
use rdev::display_size;

slint::include_modules!();

fn submit_id(id: String) {
    println!("Submitted: {}", id);

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
            submit_id(id);
            ui.window().hide().unwrap();
        }
    });

    ui.run()?;

    Ok(())
}
