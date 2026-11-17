//! The entry point behind [`crate::run`]: `beagle standup` opens the TUI;
//! the subcommands give agents a typed way to scaffold and read reports.

use std::env;
use std::ffi::OsString;
use std::process::ExitCode;

use crate::cli::{self, Command, USAGE};
use crate::model::{ReportDate, ReportManifest, TicketManifest};
use crate::store::{self, Store};
use crate::{config, today, ui, Error};

pub(crate) fn main(args: Vec<OsString>) -> ExitCode {
    let args = match cli::parse_os(args) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("error: {message}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn store_for(root_flag: Option<&str>) -> Result<Store, Error> {
    let config = match config::config_path() {
        Some(path) => config::load(&path)?,
        None => config::Config::default(),
    };
    let env_root = config::DIRS.env_root();
    let cwd = env::current_dir().map_err(|e| Error::io(".", e))?;
    Ok(Store::new(config::resolve_root(
        root_flag,
        env_root.as_deref(),
        &config,
        &cwd,
    )))
}

/// `date` parsed, or the newest report when `None`.
fn date_or_latest(store: &Store, date: Option<&str>) -> Result<ReportDate, Error> {
    match date {
        Some(d) => ReportDate::parse(d),
        None => store.latest()?.ok_or(Error::NoReports),
    }
}

fn run(args: cli::Args) -> Result<(), Error> {
    let command = args.command;
    if matches!(command, Command::Help) {
        print!("{USAGE}");
        return Ok(());
    }
    if matches!(command, Command::Guide) {
        print!("{}", crate::AGENT_GUIDE);
        return Ok(());
    }
    if matches!(command, Command::Version) {
        println!("beagle standup {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let store = store_for(args.root.as_deref())?;
    match command {
        Command::Tui { date } => {
            let date = date.as_deref().map(ReportDate::parse).transpose()?;
            let app = ui::App::new(store, date)?;
            let mut terminal = ratatui::init();
            let result = app.run(&mut terminal);
            ratatui::restore();
            result
        }
        Command::New { date, since, title } => {
            let date = date
                .as_deref()
                .map_or_else(|| Ok(today::today()), ReportDate::parse)?;
            let since = match since {
                Some(s) => Some(ReportDate::parse(&s)?),
                None => store.previous(date)?,
            };
            let manifest = ReportManifest {
                title,
                since: since.map(|d| d.to_string()),
                created: Some(today::now_rfc3339()),
            };
            let dir = store.create_report(date, &manifest)?;
            println!("{}", dir.display());
            Ok(())
        }
        Command::Ticket {
            id,
            title,
            date,
            url,
            state,
            systems,
            prs,
            update,
        } => {
            let date = date_or_latest(&store, date.as_deref())?;
            let title = title.ok_or(Error::Tool {
                tool: "ticket",
                message: "--title is required".to_owned(),
            })?;
            let manifest = TicketManifest {
                id,
                title,
                url,
                state,
                systems,
                prs,
            };
            let dir = store.upsert_ticket(date, &manifest, update)?;
            println!("{}", dir.display());
            Ok(())
        }
        Command::List { json } => list(&store, json),
        Command::Show { date } => {
            let date = date_or_latest(&store, date.as_deref())?;
            print!("{}", store::export_markdown(&store.load(date)?));
            Ok(())
        }
        Command::Last => {
            println!("{}", store.latest()?.ok_or(Error::NoReports)?);
            Ok(())
        }
        Command::Path { date } => {
            let path = match date {
                Some(d) => store.report_dir(ReportDate::parse(&d)?),
                None => store.reports_dir(),
            };
            println!("{}", path.display());
            Ok(())
        }
        Command::Help | Command::Guide | Command::Version => Ok(()),
    }
}

fn list(store: &Store, json: bool) -> Result<(), Error> {
    let rows = store.list()?;
    if json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
    } else if rows.is_empty() {
        println!("no reports in {}", store.reports_dir().display());
    } else {
        for r in rows {
            let since = r.since.map(|s| format!("since {s}")).unwrap_or_default();
            let title = r.title.unwrap_or_default();
            println!("{}  {:>2} tickets  {since:<16} {title}", r.date, r.tickets);
        }
    }
    Ok(())
}
