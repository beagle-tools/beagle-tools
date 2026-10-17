//! Agentdesk binary entry point; everything lives in `agentdesk_lib`.

// Prevents an extra console window on Windows in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    agentdesk_lib::run();
}
