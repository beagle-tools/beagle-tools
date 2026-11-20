//! Local SQLite cache for Tandem.
//!
//! The UI always renders from this cache first, then background syncs
//! refresh it — that's what makes the app feel instant. It also owns the
//! data GitHub never sees: local review comments, agent specs and runs.
//!
//! Values are stored as JSON blobs with indexed key columns. That trades
//! some queryability for schema flexibility, which is the right call
//! while the domain types are still moving.

mod agent_env;
mod archive;
mod queue;
#[cfg(test)]
mod review_runs_tests;
mod review_store;
#[cfg(test)]
mod review_store_tests;
mod reviewed;
mod revisions;
mod schema;
#[cfg(test)]
mod schema_tests;
mod store;
mod summaries;
mod triage;

pub use archive::{purge_deadline, ArchiveStore, PurgedRuns};
pub use queue::{ReviewJob, ReviewQueue};
pub use review_store::ReviewStore;
pub use reviewed::ReviewedPrs;
pub use revisions::CommentRevisions;
pub use store::Cache;
pub use summaries::SummaryStore;
pub use triage::{JobRow, PrMark, ReviewTriage};
