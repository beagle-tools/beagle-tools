//! `beagle serve`: the beagle tools' stores as a read-only HTTPS JSON API,
//! for the phone app.
//!
//! The server listens on the Mac's Tailscale address with the certificate
//! `tailscale cert` issues, and answers only requests carrying the bearer
//! token from `~/.config/beagle/serve.toml`. The API is documented in
//! `docs/serve-api.md` at the repo root; that file is the contract.
//!
//! Module layering (dependencies point downward):
//!
//! ```text
//! run ──▶ cli
//!  └──▶ server ──▶ router ──▶ api::*  (each tool through its own crate)
//!         │          ├──▶ auth
//!         │          └──▶ stores (each tool's root, resolved once)
//!         ├──▶ http   (request head parsing, responses)
//!         └──▶ tls    (rustls from the tailscale cert)
//! config, tailscale, pair (token + QR), service (launchd): used by run
//! push: phones + settings (written by api::devices), the event-spool
//!       follower and matcher, on its own background thread
//! ```
//!
//! Push notifications are specified in `docs/serve-push.md`.
//!
//! The crate follows the `beagle` tool contract: [`ABOUT`] and [`run`].
//! It is synchronous on purpose: one user, a handful of requests a minute,
//! one short-lived thread per connection.

pub mod api;
pub mod auth;
mod cli;
pub mod config;
pub mod error;
pub mod http;
pub mod pair;
pub mod push;
pub mod router;
mod run;
pub mod server;
pub mod service;
pub mod stores;
pub mod tailscale;
pub mod tls;

pub use error::{Error, Result};

/// One line for `beagle help`.
pub const ABOUT: &str = "serve every tool's data read-only over HTTPS, for the phone app";

/// Runs `beagle serve` with the arguments after `serve` (`--help` prints
/// the usage). Returns 2 on a usage error, 1 when the command fails.
pub fn run(args: Vec<std::ffi::OsString>) -> std::process::ExitCode {
    run::main(args)
}
