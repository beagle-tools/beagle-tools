//! `status`, `log` and `lock`, and the checklist lines `on` and `status`
//! print.

use std::fmt;
use std::fs;
use std::io::Write;

use crate::host::{Host, Svc};
use crate::procs::{alive, output, Target};
use crate::session::{io_err, now, Ctx, Res};
use crate::state::{fmt_dur, tail, LOG_FILE};

/// How a check came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mark {
    Ok,
    Warn,
    Fail,
}

/// One checklist line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    pub mark: Mark,
    pub text: String,
}

impl Line {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            mark: Mark::Ok,
            text: text.into(),
        }
    }

    pub fn warn(text: impl Into<String>) -> Self {
        Self {
            mark: Mark::Warn,
            text: text.into(),
        }
    }

    pub fn fail(text: impl Into<String>) -> Self {
        Self {
            mark: Mark::Fail,
            text: text.into(),
        }
    }
}

impl fmt::Display for Line {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mark = match self.mark {
            Mark::Ok => "✓",
            Mark::Warn => "⚠",
            Mark::Fail => "✗",
        };
        write!(f, "{mark} {}", self.text)
    }
}

/// agentd: running, or started now when it is installed but down.
pub(crate) fn agentd_line(host: &dyn Host) -> Line {
    match host.agentd() {
        Ok(Svc::Running) => Line::ok("agentd: running"),
        Ok(Svc::NotInstalled) => {
            Line::warn("agentd: not installed — `beagle agent daemon install` (for scheduled jobs)")
        }
        Ok(Svc::Stopped) => match host.agentd_start() {
            Ok(()) if host.agentd() == Ok(Svc::Running) => Line::ok("agentd: started"),
            _ => Line::warn("agentd: stopped — run `beagle agent daemon install` to restart it"),
        },
        Err(e) => Line::warn(format!("agentd: {e}")),
    }
}

/// One `⚠` per exposure warning, or one `✓`.
pub(crate) fn exposure_lines(warnings: &[String]) -> Vec<Line> {
    if warnings.is_empty() {
        return vec![Line::ok(
            "exposure: nothing found answering beyond this Mac",
        )];
    }
    warnings
        .iter()
        .map(|w| Line::warn(format!("exposure: {w}")))
        .collect()
}

impl Ctx {
    pub(crate) fn status(&self, out: &mut dyn Write) -> Res {
        let w = io_err("write");
        let now = now();
        let expired = self
            .active()
            .and_then(|d| d.read_i64("until"))
            .is_some_and(|until| until <= now);
        let unfinished = self.active().is_none()
            && (self.state.read("serve").is_some() || self.state.read("on_stop").is_some());
        if expired || unfinished {
            // The timer didn't get to run `off` (the Mac slept through it,
            // or restarted): finish the stop now.
            writeln!(out, "server mode had ended — finishing the stop").map_err(&w)?;
            return self.off(out);
        }
        let Some(dir) = self.active() else {
            return writeln!(out, "server mode: off").map_err(&w);
        };
        let since = dir.read_i64("since").unwrap_or(now);
        writeln!(out, "server mode: on (for {})", fmt_dur(now - since)).map_err(&w)?;
        if let Some(until) = dir.read_i64("until") {
            let clock = self
                .clock(until)
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();
            writeln!(out, "  ends in {}{clock}", fmt_dur(until - now)).map_err(&w)?;
        } else {
            writeln!(out, "  no end time — runs until 'beagle server-mode off'").map_err(&w)?;
        }
        if let Some(file) = dir.read("file") {
            match dir.read_pid("startup.pid") {
                Some(sp) if alive(&self.progs, Target::Pid(sp)) => {
                    writeln!(out, "  startup file: {file} (running, pid {sp})").map_err(&w)?;
                }
                _ => writeln!(
                    out,
                    "  startup file: {file} (exited — see 'beagle server-mode log')"
                )
                .map_err(&w)?,
            }
        }
        // Only this tool's sessions start beagle serve.
        let serve = dir == &self.state && dir.read("serve").is_some();
        for line in self.checks(serve) {
            writeln!(out, "  {line}").map_err(&w)?;
        }
        Ok(())
    }

    /// `Tue 18:30` for an epoch second, in local time (`date -r`).
    fn clock(&self, epoch: i64) -> Option<String> {
        let at = epoch.to_string();
        let ok = |args: &[&str]| {
            output(&self.progs.date, args)
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        ok(&["-r", &at, "+%a %H:%M"]).or_else(|| ok(&["-d", &format!("@{at}"), "+%a %H:%M"]))
    }

    pub(crate) fn log(&self, out: &mut dyn Write) -> Res {
        let dir = self
            .active()
            .filter(|d| d.has_log())
            .or_else(|| self.dirs().find(|d| d.has_log()))
            .ok_or("no startup log yet")?;
        let bytes = fs::read(dir.path(LOG_FILE)).map_err(io_err("read the log"))?;
        out.write_all(tail(&String::from_utf8_lossy(&bytes), 50).as_bytes())
            .map_err(io_err("write"))
    }

    /// Lock for real. `pmset displaysleepnow` alone only darkens the
    /// display and asks for the password only after the screen-lock delay,
    /// so it is the fallback, not the way.
    pub(crate) fn lock_screen(&self, out: &mut dyn Write) -> Res {
        let w = io_err("write");
        if self.host.lock_screen() {
            return writeln!(out, "  screen locked").map_err(&w);
        }
        let delay = self
            .host
            .lock_delay()
            .unwrap_or_else(|| "the lock delay".into());
        writeln!(
            out,
            "  couldn't lock: give your terminal Accessibility access\n  \
             (System Settings → Privacy & Security → Accessibility), then 'beagle server-mode lock'\n  \
             sleeping the display instead — it only locks after {delay}"
        )
        .map_err(&w)?;
        self.host.display_sleep();
        Ok(())
    }
}

/// The screen-lock delay in `sysadminctl -screenLock status` output: the
/// first of `<n> seconds`, `immediate` or `off`.
pub(crate) fn lock_delay(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    words.iter().enumerate().find_map(|(i, w)| match *w {
        "immediate" | "off" => Some((*w).to_owned()),
        n if n.chars().all(|c| c.is_ascii_digit())
            && words.get(i + 1).is_some_and(|s| s.starts_with("seconds")) =>
        {
            Some(format!("{n} seconds"))
        }
        _ => None,
    })
}

/// The epoch second in `openssl x509 -enddate` output
/// (`notAfter=Feb 25 12:00:00 2027 GMT`).
pub(crate) fn parse_enddate(text: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let rest = text.trim().strip_prefix("notAfter=")?;
    let parts: Vec<&str> = rest.split_whitespace().collect();
    let [mon, day, hms, year, "GMT"] = parts.as_slice() else {
        return None;
    };
    let month = MONTHS.iter().position(|m| m == mon)?;
    let month = time::Month::try_from(u8::try_from(month + 1).ok()?).ok()?;
    let date = time::Date::from_calendar_date(year.parse().ok()?, month, day.parse().ok()?).ok()?;
    let mut hms = hms.split(':').map(str::parse::<u8>);
    let t = time::Time::from_hms(hms.next()?.ok()?, hms.next()?.ok()?, hms.next()?.ok()?).ok()?;
    Some(date.with_time(t).assume_utc().unix_timestamp())
}
