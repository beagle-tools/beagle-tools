//! Agentdesk's Tauri shell: state init, the store poller, and command
//! registration.

// `#[tauri::command]` DESERIALIZES its arguments, so a command has to take
// them by value — `&str` or `&BTreeMap` does not compile through the
// macro, and `State<'_, T>` is already a handle rather than the thing.
// clippy cannot see through the macro expansion, so it fires on every
// command in the crate. Allowed once here rather than annotated onto each
// of the nine signatures.
#![allow(clippy::needless_pass_by_value)]

mod commands;
mod error;
mod poller;
mod state;

use commands::{dag, jobs, links, logs, pipelines, runs, updates};
use state::AppState;

/// Start Agentdesk: open the store, start the poller, and run the Tauri
/// event loop until the window closes. Exits the process (status 1) if
/// Tauri fails to start.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    beagle_utils::log::init("info");

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // `relaunch()` after an in-app update or version switch.
        .plugin(tauri_plugin_process::init())
        // Persists window size/position, including across the updater's
        // relaunch. Must register before windows are created.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            // Signed self-update against the `beagle-workflows-latest`
            // mirror (endpoint + pubkey in tauri.conf.json).
            #[cfg(desktop)]
            tauri::Manager::app_handle(app).plugin(tauri_plugin_updater::Builder::new().build())?;
            let state = AppState::init()?;
            tauri::Manager::manage(app, state);
            poller::spawn(tauri::Manager::app_handle(app).clone());
            // A dev window and the installed app read the SAME store —
            // there is only one agentd — so the title is the only thing
            // telling them apart. Worth having when both are open.
            #[cfg(debug_assertions)]
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                let _ = window.set_title("Agentdesk Dev");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            runs::list_runs,
            dag::run_forest,
            dag::agent_graph,
            dag::read_manifest,
            runs::get_run,
            runs::run_chain,
            runs::kill_run,
            runs::retry_run,
            logs::read_transcript,
            jobs::list_jobs,
            pipelines::list_pipelines,
            pipelines::pipeline_detail,
            jobs::trigger_job,
            links::run_links,
            links::read_markdown,
            updates::list_app_releases,
            updates::install_app_version,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!(error = %e, "agentdesk failed to start");
        std::process::exit(1);
    }
}
