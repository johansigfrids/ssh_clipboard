#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod connection;
mod model;
mod notifications;
mod panel;
mod shortcuts;

fn main() {
    panel::run();
}
