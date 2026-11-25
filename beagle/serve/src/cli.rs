//! Argument parsing. Pure: no filesystem, no environment.

use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;

/// `beagle serve --help`.
pub(crate) const USAGE: &str = "\
beagle serve — every tool's data, read-only over HTTPS, for the phone app

USAGE
  beagle serve [options]          run the server in the foreground
  beagle serve pair [--rotate] [--host <name>] [--port <n>]
                                  show the QR code the phone app scans (makes
                                  the token on first use; --rotate replaces it,
                                  logging out every paired phone)
  beagle serve install            run it in the background (launchd agent:
                                  starts at login, restarts if it exits);
                                  run again to restart it
  beagle serve uninstall          stop and remove the background service
  beagle serve status             whether the background service is running
  beagle serve push setup --key <AuthKey_….p8> --key-id <id> --team-id <id>
                                  turn on push notifications with your APNs key
                                  (copied next to serve.toml, mode 0600)
  beagle serve push test [--device <id>]
                                  send a test push to every registered phone

SERVER OPTIONS
  --addr <ip>      address to bind (default: `addr` in serve.toml, else
                   this Mac's Tailscale address from `tailscale ip -4`)
  --port <n>       port (default: `port` in serve.toml, else 8787)
  --any-addr       allow binding 0.0.0.0 / :: (every interface); refused
                   without it
  --cert <file>    TLS certificate, PEM (default: `cert` in serve.toml, else
                   ~/.config/beagle/serve/<host>.crt)
  --key <file>     TLS private key, PEM (default: `key` in serve.toml, else
                   ~/.config/beagle/serve/<host>.key)
  --no-tls         plain HTTP, for local testing; loopback addresses only

SETUP
  1. tailscale cert --cert-file ~/.config/beagle/serve/<host>.crt \\
       --key-file ~/.config/beagle/serve/<host>.key <host>
  2. beagle serve pair            (scan the QR code with the app)
  3. beagle serve install

Every request but GET /v1/health needs `Authorization: Bearer <token>`.
Config: ~/.config/beagle/serve.toml (0600). API: docs/serve-api.md.
";

/// How to run the server.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ServeOptions {
    pub addr: Option<IpAddr>,
    pub port: Option<u16>,
    pub any_addr: bool,
    pub cert: Option<PathBuf>,
    pub key: Option<PathBuf>,
    pub no_tls: bool,
}

/// `beagle serve pair`'s options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct PairOptions {
    pub rotate: bool,
    pub host: Option<String>,
    pub port: Option<u16>,
}

/// `beagle serve push …`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PushCommand {
    Setup {
        key: PathBuf,
        key_id: String,
        team_id: String,
    },
    Test {
        device: Option<String>,
    },
}

/// One `beagle serve` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    Serve(ServeOptions),
    Pair(PairOptions),
    Install,
    Uninstall,
    Status,
    Push(PushCommand),
}

fn port(raw: &str) -> Result<u16, String> {
    raw.parse()
        .ok()
        .filter(|p| *p != 0)
        .ok_or_else(|| format!("not a port: {raw}"))
}

/// Parses the arguments after `serve`; `Err` is a usage message.
pub(crate) fn parse(args: Vec<OsString>) -> Result<Command, String> {
    let args: Vec<String> = args
        .into_iter()
        .map(|a| {
            a.into_string()
                .map_err(|a| format!("not UTF-8: {}", a.to_string_lossy()))
        })
        .collect::<Result<_, _>>()?;
    if args.iter().any(|a| matches!(a.as_str(), "-h" | "--help")) {
        return Ok(Command::Help);
    }
    let mut it = args.into_iter().peekable();
    let bare = |command: Command, rest: &mut dyn Iterator<Item = String>| match rest.next() {
        None => Ok(command),
        Some(extra) => Err(format!("unexpected argument: {extra}")),
    };
    match it.peek().map(String::as_str) {
        Some("help") => Ok(Command::Help),
        Some("pair") => {
            it.next();
            parse_pair(&mut it)
        }
        Some("install") => {
            it.next();
            bare(Command::Install, &mut it)
        }
        Some("uninstall") => {
            it.next();
            bare(Command::Uninstall, &mut it)
        }
        Some("status") => {
            it.next();
            bare(Command::Status, &mut it)
        }
        Some("push") => {
            it.next();
            parse_push(&mut it)
        }
        _ => parse_serve(&mut it),
    }
}

fn parse_pair(it: &mut dyn Iterator<Item = String>) -> Result<Command, String> {
    let mut opts = PairOptions::default();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--rotate" => opts.rotate = true,
            "--host" => opts.host = Some(value("--host")?),
            "--port" => opts.port = Some(port(&value("--port")?)?),
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Command::Pair(opts))
}

fn parse_push(it: &mut dyn Iterator<Item = String>) -> Result<Command, String> {
    let sub = it.next();
    let mut key = None;
    let mut key_id = None;
    let mut team_id = None;
    let mut device = None;
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match (sub.as_deref(), arg.as_str()) {
            (Some("setup"), "--key") => key = Some(PathBuf::from(value("--key")?)),
            (Some("setup"), "--key-id") => key_id = Some(value("--key-id")?),
            (Some("setup"), "--team-id") => team_id = Some(value("--team-id")?),
            (Some("test"), "--device") => device = Some(value("--device")?),
            (_, other) => return Err(format!("unknown argument: {other}")),
        }
    }
    match sub.as_deref() {
        Some("setup") => match (key, key_id, team_id) {
            (Some(key), Some(key_id), Some(team_id)) => Ok(Command::Push(PushCommand::Setup {
                key,
                key_id,
                team_id,
            })),
            _ => Err("push setup needs --key, --key-id and --team-id".into()),
        },
        Some("test") => Ok(Command::Push(PushCommand::Test { device })),
        Some(other) => Err(format!("unknown push command: {other} (setup, test)")),
        None => Err("push needs a command: setup or test".into()),
    }
}

fn parse_serve(it: &mut dyn Iterator<Item = String>) -> Result<Command, String> {
    let mut opts = ServeOptions::default();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "--addr" => {
                let raw = value("--addr")?;
                opts.addr = Some(
                    raw.parse()
                        .map_err(|_| format!("not an IP address: {raw}"))?,
                );
            }
            "--port" => opts.port = Some(port(&value("--port")?)?),
            "--cert" => opts.cert = Some(PathBuf::from(value("--cert")?)),
            "--key" => opts.key = Some(PathBuf::from(value("--key")?)),
            "--any-addr" => opts.any_addr = true,
            "--no-tls" => opts.no_tls = true,
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(Command::Serve(opts))
}

#[cfg(test)]
#[path = "tests/cli.rs"]
mod tests;
