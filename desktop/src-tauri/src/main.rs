#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod connection;
mod history;
mod model;
mod notifications;
mod operation;
mod panel;
mod pending;
mod shortcuts;
mod startup_native;

fn main() {
    panel::run();
}
