//! Tauri build script: generates the app context (config, icons, ACL
//! schemas) that `tauri::generate_context!` embeds.

fn main() {
    tauri_build::build();
}
