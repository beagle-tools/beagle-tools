//! The TUI: application state machine, event loop, and key handling.
//!
//! All state lives in [`App`] and mutates only in response to key presses or
//! filesystem events — drawing (the private `view` module) is a pure function
//! of `App` except for scroll clamping. That split keeps every state
//! transition unit-testable without a terminal.
//!
//! Submodules by responsibility: `tabs` (the tab/focus enums), `event_loop`
//! (the blocking input/watcher loop), `keys` (keypress → state transition),
//! `pane` (content loading and caching), `overlays` (the toolbox and link
//! popups), `search` (in-content search), `actions` (copy/export), and
//! `view` (drawing). `App`'s own methods are grouped the same way: `filter`
//! (sidebar narrowing), `errors` (the `!` overlay), `agents` (the agents
//! screen), and `sections` (tab visibility, unread, checklists, PR states).

mod actions;
mod agents;
mod errors;
mod event_loop;
mod filter;
mod finder;
mod keys;
mod mouse;
mod overlays;
mod pane;
mod search;
mod sections;
mod settings;
mod tabs;
mod view;

pub use pane::Pane;
pub use tabs::{Focus, Screen, Tab};

use std::collections::{HashMap, HashSet};

use ratatui::text::Text;

use crate::error::Result;
use crate::model::{RcaId, RcaSummary, SectionKind};
use crate::store::{LoadWarning, Store};

use overlays::{LinksPopup, RelatedPopup};
use pane::PaneKey;
use search::ContentSearch;
use settings::SettingsOverlay;

/// The whole TUI state.
#[derive(Debug)]
#[allow(clippy::struct_excessive_bools)] // independent UI toggles (help, follow, notify, sidebar, archive), not an encoded state machine
pub struct App {
    store: Store,
    rcas: Vec<RcaSummary>,
    warnings: Vec<LoadWarning>,
    /// Workspace directories that exist but could not load — rendered at
    /// the bottom of the sidebar so they never silently disappear.
    broken: Vec<crate::store::BrokenWorkspace>,
    /// Fuzzy filter over the incident list; empty means "show everything".
    filter: String,
    /// Status facets toggled in filter mode (`i`/`r`/`v`/`f`); empty means
    /// every status passes.
    facet_statuses: HashSet<crate::model::Status>,
    /// Severity facets toggled in filter mode (`c`/`h`/`m`/`l`); empty
    /// means every severity passes.
    facet_severities: HashSet<crate::model::Severity>,
    /// `p` facet: when set, only incidents with at least one attached PR
    /// pass. Off by default (every incident passes).
    facet_has_pr: bool,
    /// `s` facet: when set, only incidents tagged `security` pass.
    facet_security: bool,
    /// Filter-mode input state: which keys the filter is capturing.
    filter_input: FilterInput,
    /// Indices into `rcas` that match `filter`, best match first.
    visible: Vec<usize>,
    /// Index into `visible` of the selected workspace, if any match.
    selected: usize,
    /// The top-level screen: the RCA browser or the agents monitor.
    screen: Screen,
    /// Latest `beagle-agentd` status from the background poller, rendered on
    /// the agents screen.
    agents_status: crate::agentd::AgentsStatus,
    /// The highlighted agent on the agents screen (index into the snapshot).
    agents_selected: usize,
    focus: Focus,
    tab: Tab,
    diagram_index: usize,
    scroll: u16,
    hscroll: u16,
    /// The content pane's line cursor: an index into the current pane's
    /// lines, used while the content is focused for line navigation and
    /// yank (`y`). Clamped to the content on every move/reload.
    content_cursor: usize,
    /// Visual-line selection anchor (`v`): `Some(line)` while a multi-line
    /// selection is active; the selection spans anchor..=cursor. `None`
    /// when not selecting.
    content_select_anchor: Option<usize>,
    show_help: bool,
    /// The `?` sheet's keybinding filter: `Some` while a query is being
    /// typed (empty string = filter active, no query yet), `None` when not
    /// filtering. Narrows the rows by fuzzy match over keys + description.
    help_filter: Option<String>,
    status: Option<String>,
    /// Content cache for the current (workspace, tab, diagram) triple.
    pane: Option<(PaneKey, Pane)>,
    /// Animation counter, advanced once per event-loop turn (the loop wakes
    /// at least every 250 ms). Drives the `investigating` spinner.
    tick: usize,
    /// Follow mode: filesystem reloads keep the current tab scrolled to the
    /// bottom, tail-f style.
    follow: bool,
    /// Sidebar collapsed (`s`): the content pane takes the full width.
    /// Never true while the list has focus — anything that returns focus
    /// to the list brings the sidebar back.
    sidebar_collapsed: bool,
    /// Archived incidents shown in the list (`a`), rendered dimmed. Off by
    /// default: the sidebar is about *now*.
    show_archived: bool,
    /// Desktop notifications on new incidents and status changes (config
    /// `notify = true`).
    notify_enabled: bool,
    /// Which lifecycle events fire a notification when `notify_enabled`
    /// (config `[notify_events]`; all events by default).
    notify_events: crate::config::NotifyEvents,
    /// Last-seen modification time per section file, for change detection.
    mtimes: HashMap<(RcaId, SectionKind), std::time::SystemTime>,
    /// Checkbox counts per section, `(checked, total)`, re-scanned only
    /// when the mtime snapshot says the file changed. Sections without
    /// checkboxes have no entry.
    checklists: HashMap<(RcaId, SectionKind), (usize, usize)>,
    /// Sections that changed on disk since the user last viewed them.
    unread: HashSet<(RcaId, SectionKind)>,
    /// Live PR states (url → state), fed by the background `gh` poller.
    /// Empty when `gh` is unavailable — PRs then render as plain links.
    pr_states: HashMap<String, crate::prs::PrState>,
    /// The `o` link popup: attached PRs plus URLs found on the current tab.
    links: Option<LinksPopup>,
    /// The `R` popup: past incidents sharing systems/tags with this one.
    related: Option<RelatedPopup>,
    /// In-content search over the pane (`/` with content focus).
    content_search: Option<ContentSearch>,
    /// The `\` global fuzzy finder; `Some` while its popup is open.
    finder: Option<finder::Finder>,
    /// The `S` settings overlay; `Some` while open.
    settings: Option<SettingsOverlay>,
    /// The `P`/`I` attach prompt (PR or ticket); `Some` while typing.
    attach_prompt: Option<overlays::AttachPrompt>,
    /// The `D` delete confirmation popup; `Some` while it awaits y/n.
    confirm_delete: Option<overlays::ConfirmDelete>,
    /// The `t` status picker; `Some` while open.
    status_picker: Option<overlays::StatusPicker>,
    /// The `#` tags editor; `Some` while open.
    tags_editor: Option<overlays::TagsEditor>,
    /// The `!` errors overlay (broken workspaces + load warnings) is open.
    show_errors: bool,
    /// Vertical scroll of the errors overlay.
    errors_scroll: u16,
    /// Geometry of the errors overlay fed back by the draw pass, for scroll
    /// clamping: (total wrapped lines, visible height).
    pub(crate) errors_viewport: (u16, u16),
    /// Rendered toolbox overlay content; `Some` while the overlay is open.
    toolbox: Option<Text<'static>>,
    /// Vertical scroll of the toolbox overlay.
    toolbox_scroll: u16,
    /// Geometry of the toolbox overlay fed back by the draw pass, for
    /// scroll clamping: (total wrapped lines, visible height).
    pub(crate) toolbox_viewport: (u16, u16),
    /// Set by the draw pass; used to clamp scrolling to real content height.
    pub(crate) viewport: ViewportInfo,
    /// Hit regions fed back by the draw pass, mapping mouse positions onto
    /// the pane / row / tab under the cursor.
    pub(crate) mouse: MouseMap,
}

/// What filter mode is doing with keystrokes: nothing (`Off`), toggling
/// facets (`Facets`, entered with `f`), or typing free text (`Typing`,
/// entered with `/` inside filter mode).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FilterInput {
    /// Filter mode closed; keys behave normally.
    Off,
    /// Single keys toggle status/severity facets.
    Facets,
    /// Keys type into the fuzzy query.
    Typing,
}

/// Where things were drawn last frame, for mouse hit-testing. Like
/// [`ViewportInfo`], this is geometry feedback from the (otherwise pure)
/// draw pass — mouse events arrive in screen coordinates and have to be
/// mapped back onto panes, sidebar rows, and tab labels.
#[derive(Debug, Clone, Default)]
pub(crate) struct MouseMap {
    /// The sidebar block, borders included. Zero-sized while collapsed.
    pub sidebar: ratatui::layout::Rect,
    /// The content body (below the header and tab bar).
    pub content: ratatui::layout::Rect,
    /// One clickable rect per tab label, in screen coordinates.
    pub tabs: Vec<(Tab, ratatui::layout::Rect)>,
    /// First visible sidebar item (the list scrolls), for row mapping.
    pub sidebar_offset: usize,
}

/// Geometry facts the draw pass feeds back for scroll clamping.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ViewportInfo {
    /// Total wrapped lines of the current content at the current width.
    pub content_lines: u16,
    /// Visible height of the content area.
    pub height: u16,
    /// Visible width of the content area (for wrap-aware search jumps).
    pub width: u16,
}

impl App {
    /// Builds the app and performs the initial workspace listing.
    ///
    /// # Errors
    /// Fails only if the `rcas/` directory itself is unreadable.
    pub fn new(store: Store) -> Result<Self> {
        let listing = store.list_all()?;
        let visible = (0..listing.summaries.len()).collect();
        let mut app = Self {
            store,
            rcas: listing.summaries,
            warnings: listing.warnings,
            broken: listing.broken,
            filter: String::new(),
            facet_statuses: HashSet::new(),
            facet_severities: HashSet::new(),
            facet_has_pr: false,
            facet_security: false,
            filter_input: FilterInput::Off,
            visible,
            selected: 0,
            screen: Screen::Rcas,
            agents_status: crate::agentd::AgentsStatus::default(),
            agents_selected: 0,
            focus: Focus::List,
            tab: Tab::Summary,
            diagram_index: 0,
            scroll: 0,
            hscroll: 0,
            content_cursor: 0,
            content_select_anchor: None,
            show_help: false,
            help_filter: None,
            status: None,
            pane: None,
            tick: 0,
            follow: false,
            sidebar_collapsed: false,
            show_archived: false,
            notify_enabled: false,
            notify_events: crate::config::NotifyEvents::all(),
            mtimes: HashMap::new(),
            checklists: HashMap::new(),
            unread: HashSet::new(),
            pr_states: HashMap::new(),
            links: None,
            related: None,
            content_search: None,
            finder: None,
            settings: None,
            attach_prompt: None,
            confirm_delete: None,
            status_picker: None,
            tags_editor: None,
            show_errors: false,
            errors_scroll: 0,
            errors_viewport: (0, 0),
            toolbox: None,
            toolbox_scroll: 0,
            toolbox_viewport: (0, 0),
            viewport: ViewportInfo::default(),
            mouse: MouseMap::default(),
        };
        // Baseline snapshot: nothing is "unread" at startup.
        app.refresh_mtimes(false);
        Ok(app)
    }

    /// The currently selected workspace, if any match the filter.
    #[must_use]
    pub fn selected_rca(&self) -> Option<&RcaSummary> {
        self.visible
            .get(self.selected)
            .and_then(|&index| self.rcas.get(index))
    }

    pub(crate) fn rcas(&self) -> &[RcaSummary] {
        &self.rcas
    }

    pub(crate) fn warnings(&self) -> &[LoadWarning] {
        &self.warnings
    }

    pub(crate) fn selected_index(&self) -> usize {
        self.selected
    }

    pub(crate) fn focus(&self) -> Focus {
        self.focus
    }

    pub(crate) fn tab(&self) -> Tab {
        self.tab
    }

    pub(crate) fn scroll_offsets(&self) -> (u16, u16) {
        (self.scroll, self.hscroll)
    }

    pub(crate) fn help_visible(&self) -> bool {
        self.show_help
    }

    /// The `?` sheet's active keybinding filter query, if one is being
    /// typed. `None` when not filtering.
    pub(crate) fn help_filter(&self) -> Option<&str> {
        self.help_filter.as_deref()
    }

    /// Closes the help sheet and drops any filter.
    pub(crate) fn close_help(&mut self) {
        self.show_help = false;
        self.help_filter = None;
    }

    pub(crate) fn status_line(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub(crate) fn tick(&self) -> usize {
        self.tick
    }

    pub(crate) fn follow(&self) -> bool {
        self.follow
    }

    pub(crate) fn sidebar_collapsed(&self) -> bool {
        self.sidebar_collapsed
    }

    pub(crate) fn show_archived(&self) -> bool {
        self.show_archived
    }

    /// How many loaded workspaces are archived.
    pub(crate) fn archived_count(&self) -> usize {
        self.rcas.iter().filter(|rca| rca.archived).count()
    }
}

/// Sets up the terminal, runs the app, and unconditionally restores the
/// terminal — including on error paths.
///
/// # Errors
/// Propagates any [`Error`](crate::Error) from app construction or the
/// event loop.
pub fn run(store: Store, notify: bool, notify_events: crate::config::NotifyEvents) -> Result<()> {
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
        app.notify_enabled = notify;
        app.notify_events = notify_events;
        app.run(&mut terminal)
    });
    let _ = crossterm::execute!(std::io::stdout(), crossterm::event::DisableMouseCapture);
    ratatui::restore();
    result
}

#[cfg(test)]
#[path = "tests/util.rs"]
pub(crate) mod testutil;
