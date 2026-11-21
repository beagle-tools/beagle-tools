//! Which gh account posts and approvals go out as.
//!
//! gh can be logged into several github.com accounts; plain `gh auth
//! token` answers with whichever is active, which may not be the one you
//! review as. So a write always names its account, and the TUI never
//! falls back to the active one:
//!
//! - `github_user` in the config picks it;
//! - else, when gh has exactly one account, that one;
//! - else nobody is chosen, and the first write (`P`, `A`) opens the
//!   picker before its confirmation.
//!
//! `u` opens the picker any time; the choice is saved to the config's
//! `github_user`. The footer always shows who the next write posts as.

use beagle_utils::gh::Account;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Constraint, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use super::worker::Cmd;
use super::{Action, App, Mode};
use crate::config;

/// Who writes go out as before anyone picks: the configured account, else
/// gh's only account. `None` (ask first) when gh has several, none, or
/// has not answered yet — never gh's active account.
#[must_use]
pub fn resolve(configured: Option<&str>, accounts: Option<&[Account]>) -> Option<String> {
    if let Some(user) = configured.filter(|u| !u.is_empty()) {
        return Some(user.to_owned());
    }
    match accounts {
        Some([only]) => Some(only.login.clone()),
        _ => None,
    }
}

impl App {
    /// gh answered with its accounts.
    pub(super) fn set_accounts(&mut self, accounts: Vec<Account>) {
        if self.account.is_none() {
            self.account = resolve(None, Some(&accounts));
        }
        let unknown = self
            .account
            .as_ref()
            .filter(|a| !accounts.iter().any(|x| &x.login == *a));
        if let Some(user) = unknown {
            self.error(format!(
                "{user} is not logged in to gh — press u to pick another, or `gh auth login`"
            ));
        }
        self.accounts = Some(accounts);
    }

    /// Asks to confirm `action`, after asking who it posts as when no
    /// account is chosen yet.
    pub(super) fn confirm_as_account(&mut self, action: Action) {
        if self.account.is_some() {
            self.mode = Mode::Confirm(action);
        } else {
            self.open_picker(Some(action));
        }
    }

    /// `u`, or a write with no account chosen: the account picker.
    pub(super) fn open_picker(&mut self, then: Option<Action>) {
        match &self.accounts {
            None => {
                self.info("still asking gh for its accounts — try again in a moment");
                self.send(Cmd::Accounts);
            }
            Some(list) if list.is_empty() => {
                self.error("gh has no github.com account logged in — run `gh auth login`");
            }
            Some(list) => {
                let sel = list
                    .iter()
                    .position(|a| Some(&a.login) == self.account.as_ref())
                    .unwrap_or(0);
                self.mode = Mode::Pick { sel, then };
            }
        }
    }

    /// A key in the picker: move, choose (then the pending write's
    /// confirmation), or cancel.
    pub(super) fn pick_key(&mut self, key: KeyEvent, sel: usize, then: Option<Action>) {
        let n = self.accounts.as_ref().map_or(0, Vec::len);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                let sel = (sel + 1).min(n.saturating_sub(1));
                self.mode = Mode::Pick { sel, then };
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let sel = sel.saturating_sub(1);
                self.mode = Mode::Pick { sel, then };
            }
            KeyCode::Enter => {
                let login = self
                    .accounts
                    .as_ref()
                    .and_then(|l| l.get(sel))
                    .map(|a| a.login.clone());
                if let Some(login) = login {
                    self.choose_account(login);
                    if let Some(action) = then {
                        self.mode = Mode::Confirm(action);
                    }
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => self.info("cancelled"),
            _ => self.mode = Mode::Pick { sel, then },
        }
    }

    /// Writes from now on go out as `login`; saved to the config's
    /// `github_user` when there is a config file to save to.
    fn choose_account(&mut self, login: String) {
        let saved = match &self.config_file {
            Some(path) => config::save_github_user_to(path, &login)
                .map(|()| format!("saved to {}", path.display())),
            None => Err("no config file to save it to".to_owned()),
        };
        match saved {
            Ok(where_) => self.info(format!("posting as {login} · {where_}")),
            Err(why) => self.error(format!("posting as {login} for this session — {why}")),
        }
        self.config.github_user = Some(login.clone());
        self.account = Some(login);
    }
}

/// The footer badge: who the next write posts as.
pub fn badge(app: &App) -> Span<'static> {
    let (text, color) = match (&app.account, &app.accounts) {
        (Some(user), _) => (format!(" as {user} "), Color::Cyan),
        (None, None) => (" as … ".to_owned(), Color::DarkGray),
        (None, Some(list)) if list.is_empty() => (" no gh account ".to_owned(), Color::LightRed),
        (None, Some(_)) => (" as ? (u to choose) ".to_owned(), Color::Yellow),
    };
    Span::styled(
        text,
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

/// The picker overlay.
pub fn draw_picker(frame: &mut Frame<'_>, app: &App, sel: usize, then: Option<&Action>) {
    let accounts = app.accounts.as_deref().unwrap_or_default();
    let title = if then.is_some() {
        " Post to GitHub as which account? "
    } else {
        " GitHub account for posts and approvals "
    };
    let height = u16::try_from(accounts.len())
        .unwrap_or(u16::MAX)
        .saturating_add(4);
    let area = super::view::centered(frame.area(), 60, height);
    frame.render_widget(Clear, area);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let [list_area, keys] =
        Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(inner);
    let items: Vec<ListItem<'_>> = accounts
        .iter()
        .map(|a| {
            let mut spans = vec![Span::raw(format!(" {}", a.login))];
            if a.active {
                spans.push(Span::styled(
                    "  gh active",
                    Style::default().fg(Color::DarkGray),
                ));
            }
            if Some(&a.login) == app.account.as_ref() {
                spans.push(Span::styled(
                    "  ◂ current",
                    Style::default().fg(Color::Cyan),
                ));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();
    let list = List::new(items).highlight_style(Style::default().bg(Color::Rgb(40, 44, 52)));
    let mut state = ListState::default().with_selected(Some(sel));
    frame.render_stateful_widget(list, list_area, &mut state);
    frame.render_widget(
        Paragraph::new(Line::styled(
            "j/k move · Enter choose (saved to the config) · Esc cancel",
            Style::default().fg(Color::Yellow),
        )),
        keys,
    );
}
