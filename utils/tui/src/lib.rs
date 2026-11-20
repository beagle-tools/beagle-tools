//! Terminal-UI pieces shared by the beagle-tools TUIs (`beagle recap`,
//! `beagle standup`, …).
//!
//! Everything here renders with `ratatui` and is app-agnostic: markdown and
//! ANSI rendering, word wrapping, a scrolling document pane with a vim-style
//! cursor ([`DocView`]), common widgets, live-reload watching, clipboard and
//! URL opening, store-root / config-file discovery, the shared side-pane
//! resize mode ([`SideResize`]), the `delta` diff runner, and the release picker
//! behind `version list`. Apps keep their own domain types and error
//! enums; errors from here convert into them.
//!
//! Module layering (dependencies point strictly downward):
//!
//! ```text
//! widgets ──▶ docview ──▶ wrap
//!   doc ──▶ markdown ──▶ wrap
//!   doc ──▶ ansi
//! docview ──▶ clipboard
//! ```

pub mod ansi;
pub mod clipboard;
pub mod config;
pub mod delta;
pub mod doc;
pub mod docview;
pub mod error;
pub mod links;
pub mod markdown;
pub mod pane_resize;
pub mod version_picker;
pub mod watch;
pub mod widgets;
pub mod wrap;

pub use docview::DocView;
pub use error::{ConfigError, ToolError};
pub use pane_resize::{DefaultWidth, PaneLimits, SideResize};
