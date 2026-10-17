//! Tandem's Tauri application shell: state init + command registration.

mod commands;
mod file_config;
mod settings;
mod state;

use commands::{
    agents, conflicts, files, github_write, mcp, merge, prs, review, settings_cmd, text_tools,
    threads, updates, workspace,
};
use state::AppState;

/// Initialise logging, build the Tauri app (plugins, state, commands),
/// and run its event loop until the last window closes. Exits the process
/// with status 1 if Tauri fails to start.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    beagle_utils::log::init("info");

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        // Persist window size/position across restarts (including the
        // updater's relaunch). Must register before windows are created.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            #[cfg(desktop)]
            tauri::Manager::app_handle(app).plugin(tauri_plugin_updater::Builder::new().build())?;
            let state = AppState::init()?;
            // Startup housekeeping: purge archive entries past their
            // 3-day EST deadline (and their run dirs).
            if let Ok(mut cache) = state.cache.try_lock() {
                if let Err(e) = commands::prs::purge_expired_data(&mut cache, &state.dirs)
                {
                    tracing::warn!(error = %e, "startup purge failed");
                }
                // Runs never survive a restart — fail out zombie rows so
                // the UI doesn't show them as running forever.
                match tandem_cache::ReviewStore::sweep_stale_runs(&*cache) {
                    Ok(0) => {}
                    Ok(n) => tracing::info!(count = n, "marked stale agent runs as failed"),
                    Err(e) => tracing::warn!(error = %e, "stale-run sweep failed"),
                }
            }
            tauri::Manager::manage(app, state);
            // Distinguish a `cargo tauri dev` window from the installed
            // release app at a glance — they already keep separate data
            // (see AppDirs::resolve), but the title makes it obvious.
            #[cfg(debug_assertions)]
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                let _ = window.set_title("Tandem Dev — code review");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            prs::get_pull_requests,
            prs::sync_pull_requests,
            prs::get_pr_bundle,
            prs::sync_pr_bundle,
            prs::get_last_synced,
            prs::list_archived_prs,
            prs::search_prs,
            prs::list_my_prs,
            prs::list_failing_checks,
            prs::list_pr_commits,
            prs::forget_pr,
            prs::list_reviewed_prs,
            files::get_file_excerpt,
            review::list_local_comments,
            review::add_local_comment,
            review::set_comment_status,
            review::update_comment_body,
            review::list_comment_revisions,
            review::delete_local_comment,
            review::delete_pr_summary,
            mcp::list_mcp_servers,
            agents::list_agent_specs,
            agents::save_agent_spec,
            agents::delete_agent_spec,
            workspace::resolve_workspace,
            workspace::open_in_editor,
            agents::list_agent_runs,
            agents::list_all_agent_runs,
            agents::get_run_log,
            agents::start_agent_review,
            agents::cancel_agent_run,
            agents::delete_agent_run,
            threads::reply_to_comment,
            threads::mention_agent,
            threads::reply_to_summary,
            conflicts::start_conflict_resolution,
            conflicts::start_conflict_fix,
            conflicts::conflict_resolution_state,
            github_write::push_conflict_resolution,
            github_write::post_comment_to_github,
            github_write::approve_pr,
            github_write::reply_on_github,
            github_write::commit_suggestion,
            github_write::resolve_github_thread,
            github_write::mark_ready_for_review,
            merge::merge_pr,
            merge::update_pr_branch,
            merge::enable_auto_merge,
            merge::repo_merge_options,
            settings_cmd::get_settings,
            settings_cmd::update_settings,
            settings_cmd::file_config_info,
            settings_cmd::list_github_owners,
            settings_cmd::list_github_repos,
            settings_cmd::list_collaborators,
            text_tools::polish_text,
            updates::list_app_releases,
            updates::install_app_version,
        ])
        .run(tauri::generate_context!());

    if let Err(e) = result {
        tracing::error!(error = %e, "tauri application failed");
        std::process::exit(1);
    }
}
