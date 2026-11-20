//! The TLS server config, from the PEM certificate and key `tailscale cert`
//! writes.

use std::path::Path;
use std::sync::Arc;

use rustls::ServerConfig;
use rustls_pki_types::pem::PemObject as _;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use crate::error::{Error, Result};

/// A rustls server config for `cert` (a PEM chain) and `key` (a PEM private
/// key), with rustls's safe default protocol versions and the ring
/// provider.
///
/// # Errors
/// [`Error::Tls`] when either file is unreadable or not valid PEM, or the
/// key does not match the certificate.
pub fn server_config(cert: &Path, key: &Path) -> Result<Arc<ServerConfig>> {
    let chain: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert)
        .and_then(Iterator::collect)
        .map_err(|e| Error::Tls(format!("{}: {e}", cert.display())))?;
    if chain.is_empty() {
        return Err(Error::Tls(format!(
            "{}: no certificate in the file",
            cert.display()
        )));
    }
    let key = PrivateKeyDer::from_pem_file(key)
        .map_err(|e| Error::Tls(format!("{}: {e}", key.display())))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| Error::Tls(e.to_string()))?
        .with_no_client_auth()
        .with_single_cert(chain, key)
        .map_err(|e| Error::Tls(format!("{}: {e}", cert.display())))?;
    Ok(Arc::new(config))
}

#[cfg(test)]
// Tests fail loudly on setup errors; unwrap is the clearest way.
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_garbage_files_are_tls_errors() {
        let dir = tempfile::tempdir().unwrap();
        let cert = dir.path().join("c.crt");
        let key = dir.path().join("c.key");
        assert!(matches!(server_config(&cert, &key), Err(Error::Tls(_))));
        std::fs::write(&cert, "not pem").unwrap();
        std::fs::write(&key, "not pem").unwrap();
        let err = server_config(&cert, &key).unwrap_err().to_string();
        assert!(err.contains("c.crt"), "{err}");
    }
}
