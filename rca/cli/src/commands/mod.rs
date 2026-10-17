//! The I/O half of each subcommand (parsing is in `crate::cli`), grouped by
//! what it touches: `workspace` (listing, init, lifecycle, read-only views),
//! `links` (attached PRs and tickets), `handoff` (piping an RCA to an
//! external agent), and `setup` (config, self-update, the agent service,
//! skills).

mod handoff;
mod links;
mod setup;
mod workspace;

pub(crate) use handoff::run_handoff;
pub(crate) use links::{run_pr_add, run_pr_list, run_ticket_add, run_ticket_list, run_ticket_sync};
pub(crate) use setup::{
    install_version, latest_release, run_agent_service, run_config, run_skill, run_version_list,
};
pub(crate) use workspace::{
    run_archive, run_context, run_init, run_list, run_set_published, run_set_status, run_similar,
    run_unarchive,
};
