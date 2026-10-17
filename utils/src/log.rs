//! `tracing` subscriber setup (requires the `tracing` feature).

use tracing_subscriber::EnvFilter;

/// Installs a global `fmt` subscriber filtered by `$RUST_LOG`, or by
/// `default_directive` (e.g. `"info"` or `"my_app=debug"`) when `$RUST_LOG`
/// is unset or invalid.
///
/// Safe to call more than once: if a global subscriber is already set, this
/// is a no-op rather than a panic.
pub fn init(default_directive: &str) {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_directive));
    // The only error is "a global subscriber is already set", which is fine:
    // whoever set it first wins, and logging still works.
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_twice_does_not_panic() {
        init("info");
        init("debug");
        tracing::info!("logging initialised");
    }
}
