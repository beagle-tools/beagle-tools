//! Entry point for the interactive TUI: terminal setup and teardown, plus
//! the config-driven [`Settings`] the caller resolves before launch.

use crate::error::Result;
use crate::store::Store;

use super::App;

/// Config-driven TUI settings, resolved by the caller.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Desktop notifications on (config `notify`).
    pub notify: bool,
    /// Which events notify (config `[notify_events]`).
    pub notify_events: crate::config::NotifyEvents,
    /// Display zone (config `timezone`); an `Err` names a zone that didn't
    /// parse, shown on the status line while timestamps fall back to UTC.
    pub timezone: std::result::Result<crate::tz::TimeZone, String>,
    /// 12- or 24-hour clock (config `clock`).
    pub clock: crate::tz::Clock,
}

impl Settings {
    /// Resolves settings from the effective config, with defaults for
    /// anything unset: notifications off, every event when `[notify_events]`
    /// is absent, UTC, 24-hour clock. A bad zone is kept as the `Err` so the
    /// TUI can say why it fell back to UTC rather than refusing to start.
    #[must_use]
    pub fn from_config(config: Option<&crate::config::Config>) -> Self {
        Self {
            notify: config.and_then(|c| c.notify).unwrap_or(false),
            notify_events: config
                .and_then(|c| c.notify_events.clone())
                .unwrap_or_else(crate::config::NotifyEvents::all),
            timezone: config.and_then(|c| c.timezone.as_deref()).map_or_else(
                || Ok(crate::tz::TimeZone::utc()),
                crate::tz::TimeZone::parse,
            ),
            clock: config.and_then(|c| c.clock).unwrap_or_default(),
        }
    }
}

impl App {
    /// Applies launch settings. A zone that didn't parse leaves UTC in
    /// place and explains why on the status line instead of refusing to
    /// start.
    pub(crate) fn apply_settings(&mut self, settings: Settings) {
        self.notify_enabled = settings.notify;
        self.notify_events = settings.notify_events;
        self.clock = settings.clock;
        match settings.timezone {
            Ok(zone) => self.timezone = zone,
            Err(e) => self.status = Some(format!("config: {e}; showing UTC")),
        }
    }
}

/// Sets up the terminal, runs the app, and unconditionally restores the
/// terminal — including on error paths.
///
/// # Errors
/// Propagates any [`Error`](crate::Error) from app construction or the
/// event loop.
pub fn run(store: Store, settings: Settings) -> Result<()> {
    let mut terminal = ratatui::init();
    // Mouse capture must be torn down on every exit path — left on, it
    // garbles the user's shell (scrolling emits escape codes). The panic
    // hook wraps the one `ratatui::init` installed, so capture is released
    // before the terminal is restored even when we panic.
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::EnableMouseCapture);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
        previous_hook(info);
    }));
    let result = App::new(store).and_then(|mut app| {
        app.apply_settings(settings);
        app.run(&mut terminal)
    });
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
#[path = "tests/launch.rs"]
mod tests;
