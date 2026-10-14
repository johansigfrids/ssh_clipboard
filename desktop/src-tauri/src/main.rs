#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod connection;
mod model;
mod panel;

fn main() {
    panel::run();
}
