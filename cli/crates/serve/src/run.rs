//! The commands' I/O: config, Tailscale, TLS, then the listener.

use std::ffi::OsString;
use std::net::{IpAddr, SocketAddr, TcpListener};
use std::process::ExitCode;
use std::sync::Arc;

use crate::auth::Token;
use crate::cli::{self, Command, ServeOptions};
use crate::config;
use crate::error::{Error, Result};
use crate::router::Ctx;
use crate::stores::Stores;
use crate::{server, tailscale, tls};

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
    server::serve(&listener, &Arc::new(Ctx::new(token, stores)), tls.as_ref());
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
