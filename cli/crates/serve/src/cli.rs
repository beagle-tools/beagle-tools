//! Argument parsing. Pure: no filesystem, no environment.

use std::ffi::OsString;
use std::net::IpAddr;
use std::path::PathBuf;

/// `beagle serve --help`.
pub(crate) const USAGE: &str = "\
beagle serve — every tool's data, read-only over HTTPS, for the phone app

USAGE
  beagle serve [options]          run the server in the foreground

OPTIONS
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

/// One `beagle serve` invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Command {
    Help,
    Serve(ServeOptions),
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
    let mut it = args.into_iter();
    let mut opts = ServeOptions::default();
    while let Some(arg) = it.next() {
        let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
        match arg.as_str() {
            "-h" | "--help" | "help" => return Ok(Command::Help),
            "--addr" => {
                let raw = value("--addr")?;
                opts.addr = Some(
                    raw.parse()
                        .map_err(|_| format!("not an IP address: {raw}"))?,
                );
            }
            "--port" => {
                let raw = value("--port")?;
                opts.port = Some(
                    raw.parse()
                        .ok()
                        .filter(|p| *p != 0)
                        .ok_or_else(|| format!("not a port: {raw}"))?,
                );
            }
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
// Tests fail loudly on setup errors; unwrap is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn p(args: &[&str]) -> Result<Command, String> {
        parse(args.iter().map(OsString::from).collect())
    }

    #[test]
    fn no_arguments_serves_with_defaults() {
        assert_eq!(p(&[]).unwrap(), Command::Serve(ServeOptions::default()));
    }

    #[test]
    fn every_option_parses() {
        let cmd = p(&[
            "--addr",
            "127.0.0.1",
            "--port",
            "9000",
            "--any-addr",
            "--no-tls",
            "--cert",
            "c",
            "--key",
            "k",
        ])
        .unwrap();
        assert_eq!(
            cmd,
            Command::Serve(ServeOptions {
                addr: Some("127.0.0.1".parse().unwrap()),
                port: Some(9000),
                any_addr: true,
                cert: Some("c".into()),
                key: Some("k".into()),
                no_tls: true,
            })
        );
    }

    #[test]
    fn help_and_mistakes() {
        assert_eq!(p(&["--help"]).unwrap(), Command::Help);
        assert!(p(&["--addr"]).unwrap_err().contains("needs a value"));
        assert!(p(&["--addr", "mac"]).unwrap_err().contains("not an IP"));
        assert!(p(&["--port", "0"]).unwrap_err().contains("not a port"));
        assert!(p(&["--port", "70000"]).unwrap_err().contains("not a port"));
        assert!(p(&["--bogus"]).unwrap_err().contains("unknown argument"));
    }
}
