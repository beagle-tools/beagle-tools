//! Beagle desktop: a Tauri shell over the CLI crate's domain layer.
//!
//! Modules: `dto` (the IPC contract, mirrored by `src/types.ts`), `commands`
//! (the invoke surface), `error` (the typed command error, serialized as a
//! plain message string), `agents` (the beagle-agentd control proxy + live
//! status emitter), and `updates` (release listing + version install for the
//! in-app updater). This file only assembles the app.

mod agents;
mod commands;
mod dto;
mod error;
mod updates;

/// Builds and runs the Tauri application.
///
/// # Panics
/// Panics only if the webview cannot start at all — there is no UI to
/// degrade to at that point.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(clippy::expect_used)] // no UI exists yet to report through
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            #[cfg(desktop)]
            app.handle()
                .plugin(tauri_plugin_updater::Builder::new().build())?;
            // Stream live beagle-agentd status to the frontend for its lifetime.
            agents::spawn_status_emitter(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_workspaces,
            commands::read_section,
            commands::list_diagrams,
            commands::read_diagram,
            commands::archive_workspace,
            commands::unarchive_workspace,
            commands::search_corpus,
            commands::add_pr,
            commands::pr_states,
            agents::agents_status,
            agents::start_agent,
            agents::stop_agent,
            agents::reload_agents_config,
            updates::list_app_releases,
            updates::install_app_version
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
