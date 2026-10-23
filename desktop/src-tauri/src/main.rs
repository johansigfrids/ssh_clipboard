#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod clipboard_payload;
mod configuration;
mod connection;
mod file_cache;
mod history;
mod model;
mod notifications;
mod operation;
mod panel;
mod pending;
mod shortcuts;
mod startup_native;
mod updates;

fn main() {
    panel::run();
}
