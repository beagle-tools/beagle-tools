//! The `Beagle` desktop binary: a thin entry point over [`beagle_desktop_lib::run`].

// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    beagle_desktop_lib::run();
}
