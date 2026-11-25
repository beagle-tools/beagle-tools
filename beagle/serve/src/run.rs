//! The commands' I/O: config, Tailscale, TLS, then the listener.

use std::ffi::OsString;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::process::ExitCode;
use std::sync::Arc;

use crate::auth::Token;
use crate::cli::{self, Command, PairOptions, PushCommand, ServeOptions};
use crate::config;
use crate::conn::{Connections, Limits};
use crate::error::{Error, Result};
use crate::push::devices::{self, DeviceStore};
use crate::router::Ctx;
use crate::stores::Stores;
use crate::{pair, push, server, service, tailscale, tls};

/// Entry point behind [`crate::run`].
pub(crate) fn main(args: Vec<OsString>) -> ExitCode {
    let command = match cli::parse(args) {
        Ok(command) => command,
        Err(message) => {
            eprintln!("beagle serve: {message}\nrun `beagle serve --help` for usage");
            return ExitCode::from(2);
        }
    };
    let result = match command {
        Command::Help => {
            print!("{}", cli::USAGE);
            Ok(())
        }
        Command::Serve(opts) => serve(&opts),
        Command::Pair(opts) => pair_command(&opts),
        Command::Install => service::install().map(|done| println!("{done}")),
        Command::Uninstall => service::uninstall().map(|done| println!("{done}")),
        Command::Status => status(),
        Command::Push(PushCommand::Setup {
            key,
            key_id,
            team_id,
        }) => push::cmd::setup(&key, &key_id, &team_id),
        Command::Push(PushCommand::Test { device }) => push::cmd::test(device.as_deref()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("beagle serve: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The config file's settings, warning when its permissions are loose.
fn load_config() -> Result<(std::path::PathBuf, config::Config)> {
    let path = config::path()?;
    if let Some(warning) = config::permission_warning(&path) {
        eprintln!("beagle serve: warning: {warning}");
    }
    let config = config::load(&path)?;
    Ok((path, config))
}

/// `beagle serve pair`: makes (or rotates) the token, saves what it
/// learned, and prints the QR code and the plain values.
fn pair_command(opts: &PairOptions) -> Result<()> {
    let (path, mut config) = load_config()?;
    let (token, new) = pair::token_for(&config, opts.rotate)?;
    let host = match opts.host.clone().or_else(|| config.host.clone()) {
        Some(host) => host,
        None => tailscale::dns_name()?,
    };
    let port = opts.port.unwrap_or_else(|| config.port());
    let updated = config::Config {
        token: Some(token.as_str().to_owned()),
        host: Some(host.clone()),
        port: opts.port.or(config.port),
        ..config.clone()
    };
    if updated != config {
        config::save(&path, &updated)?;
        config = updated;
    }
    let url = pair::server_url(&host, port);
    println!("{}", pair::qr(&pair::pair_link(&url, &token))?);
    println!("Scan this with the beagle app, or enter:");
    println!("  server: {url}");
    println!("  token:  {}", token.as_str());
    if new {
        println!("\nNew token saved to {} (mode 0600).", path.display());
        // A new token logs every phone out, so none should get pushes.
        devices::clear(&push::devices_path()?)?;
        restart_after_rotation();
    }
    let (cert, key) = config.cert_paths(&config::cert_dir()?, &host);
    if !cert.is_file() || !key.is_file() {
        println!(
            "\nNo TLS certificate yet. Create it with:\n  tailscale cert --cert-file {} --key-file {} {host}",
            cert.display(),
            key.display()
        );
    }
    Ok(())
}

/// A running server read the old token at startup; restart the background
/// service so the new one takes effect.
fn restart_after_rotation() {
    if !cfg!(target_os = "macos") {
        println!("Restart `beagle serve` if it is running, so it uses the new token.");
        return;
    }
    match service::restart_if_loaded() {
        Ok(true) => println!("Restarted the background service: it now uses the new token."),
        Ok(false) => println!("Restart `beagle serve` if it is running, so it uses the new token."),
        Err(e) => eprintln!("beagle serve: warning: could not restart the service: {e}"),
    }
}

/// `beagle serve status`.
fn status() -> Result<()> {
    println!("{}", service::status()?.describe());
    let (_, config) = load_config()?;
    if let Some(host) = &config.host {
        println!("  url:   {}", pair::server_url(host, config.port()));
    }
    println!(
        "  token: {}",
        if config.token.is_some() {
            "set"
        } else {
            "none — run `beagle serve pair`"
        }
    );
    let phones = DeviceStore::new(push::devices_path()?).load().map_or_else(
        |e| format!("unreadable ({e})"),
        |d| d.devices.len().to_string(),
    );
    println!("  phones registered for push: {phones}");
    println!("  push:  {}", push::status().unwrap_or_else(|why| why));
    Ok(())
}

/// `beagle serve [options]`: runs until killed.
fn serve(opts: &ServeOptions) -> Result<()> {
    let (path, config) = load_config()?;
    let token = config.token.as_deref().ok_or(Error::NoToken)?;
    let token = Token::parse(token).ok_or_else(|| Error::BadToken(path.clone()))?;
    let ip = match opts.addr.or(config.addr) {
        Some(ip) => ip,
        None => IpAddr::V4(tailscale::ipv4()?),
    };
    check_bind(ip, opts.any_addr, opts.no_tls)?;
    let port = opts.port.unwrap_or_else(|| config.port());
    let addr = SocketAddr::new(ip, port);
    let (tls, url) = if opts.no_tls {
        (None, format!("http://{addr}"))
    } else {
        let host = match &config.host {
            Some(host) => host.clone(),
            None => tailscale::dns_name()?,
        };
        let dir = config::cert_dir()?;
        let (default_cert, default_key) = config.cert_paths(&dir, &host);
        let cert = opts.cert.clone().unwrap_or(default_cert);
        let key = opts.key.clone().unwrap_or(default_key);
        if !cert.is_file() || !key.is_file() {
            return Err(Error::MissingCert {
                cert,
                key,
                dir,
                host,
            });
        }
        (
            Some(tls::server_config(&cert, &key)?),
            format!("https://{host}:{port}"),
        )
    };
    let listener = TcpListener::bind(addr).map_err(|source| Error::Bind { addr, source })?;
    let stores = Stores::resolve();
    eprintln!("beagle serve: listening on {url} (bound to {addr})");
    for line in stores.describe() {
        eprintln!("beagle serve:   {line}");
    }
    let devices = Arc::new(DeviceStore::new(push::devices_path()?));
    // A broken [push] turns push off with a log line; it never stops serving.
    let sender = push::sender_for(&config, Arc::clone(&devices)).unwrap_or_else(|why| {
        eprintln!("beagle serve:   push: off: {why}");
        Box::new(push::HeldSender)
    });
    eprintln!(
        "beagle serve:   push: {}; phones in {}",
        if sender.configured() {
            "on (APNs)"
        } else {
            "not configured, events are only logged"
        },
        devices.path().display()
    );
    let sender: push::SharedSender = Arc::new(std::sync::Mutex::new(sender));
    push::spawn(Arc::clone(&devices), Arc::clone(&sender))?;
    let limits = Limits::default();
    let conns = Connections::new(limits.max);
    let ctx = Ctx::new(token, stores)
        .with_devices(devices)
        .with_push(sender)
        .with_connections(conns.clone());
    server::serve(&listener, &Arc::new(ctx), tls.as_ref(), &conns, limits);
    Ok(())
}

/// Refuses the two dangerous binds: every interface without `--any-addr`,
/// and plain HTTP anywhere but loopback.
///
/// # Errors
/// [`Error::Refused`] saying which flag would allow it, or why not.
pub(crate) fn check_bind(ip: IpAddr, any_addr: bool, no_tls: bool) -> Result<()> {
    if ip.is_unspecified() && !any_addr {
        return Err(Error::Refused(format!(
            "{ip} listens on every interface, not just Tailscale: pass --any-addr if you mean it"
        )));
    }
    if no_tls && !ip.is_loopback() {
        return Err(Error::Refused(format!(
            "--no-tls would send the token in clear text: only allowed on loopback (127.0.0.1 or ::1), not {ip}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(raw: &str) -> IpAddr {
        raw.parse()
            .unwrap_or(IpAddr::V4(std::net::Ipv4Addr::BROADCAST))
    }

    #[test]
    fn every_interface_needs_the_flag() {
        assert!(check_bind(ip("0.0.0.0"), false, false).is_err());
        assert!(check_bind(ip("::"), false, false).is_err());
        assert!(check_bind(ip("0.0.0.0"), true, false).is_ok());
        assert!(check_bind(ip("100.64.0.1"), false, false).is_ok());
    }

    #[test]
    fn plain_http_only_on_loopback() {
        assert!(check_bind(ip("127.0.0.1"), false, true).is_ok());
        assert!(check_bind(ip("::1"), false, true).is_ok());
        assert!(check_bind(ip("100.64.0.1"), false, true).is_err());
        assert!(check_bind(ip("0.0.0.0"), true, true).is_err());
    }
}
