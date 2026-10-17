//! Tauri build script: generates the app context (config, icons, capability
//! ACLs) that `tauri::generate_context!` embeds at compile time.

fn main() {
    tauri_build::build();
}
