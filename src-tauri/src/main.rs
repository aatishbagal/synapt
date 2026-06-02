#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod network;
mod trust;
mod storage;
mod platform;

fn main() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error running Tauri application");
}
