// A release build on Windows opens its window without a console behind it.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    surco_app::run()
}
