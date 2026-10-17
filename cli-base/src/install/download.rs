//! Download and sha256 verification of a release tarball.

use std::path::Path;

use crate::error::{Error, Result};
use crate::tool;

/// Downloads `url` to `to` with curl (`file://` works too, which is how the
/// pipeline is tested without a network).
pub fn download(url: &str, to: &Path) -> Result<()> {
    let mut args = tool::curl_args();
    args.extend([
        "-o".to_owned(),
        to.to_string_lossy().into_owned(),
        url.to_owned(),
    ]);
    tool::run("curl", &args, tool::DOWNLOAD_TIMEOUT).map(|_| ())
}

/// Checks `tarball` against the digest in `sha_file` (`<hex>  <name>`), and
/// returns the digest.
pub fn verify(tarball: &Path, sha_file: &Path, asset: &str) -> Result<String> {
    let raw = std::fs::read_to_string(sha_file).map_err(|e| Error::io(sha_file, e))?;
    let expected = parse_sha(&raw).ok_or_else(|| Error::Tool {
        tool: "sha256",
        message: format!("`{}` does not contain a sha256 digest", sha_file.display()),
    })?;
    let actual = sha256_of(tarball)?;
    if expected == actual {
        Ok(actual)
    } else {
        Err(Error::ChecksumMismatch {
            asset: asset.to_owned(),
            expected,
            actual,
        })
    }
}

/// The leading 64-hex-char digest of a checksum line, lowercased.
pub fn parse_sha(line: &str) -> Option<String> {
    let token = line.split_whitespace().next()?;
    (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| token.to_ascii_lowercase())
}

/// A file's sha256 via `shasum -a 256` (macOS) or `sha256sum` (Linux),
/// whichever exists.
pub fn sha256_of(path: &Path) -> Result<String> {
    let path = path.to_string_lossy();
    let out = match tool::run("shasum", &["-a", "256", &path], tool::LOCAL_TIMEOUT) {
        Ok(out) => out,
        Err(Error::Tool { message, .. }) if message.contains("not found on PATH") => {
            tool::run("sha256sum", &[path.as_ref()], tool::LOCAL_TIMEOUT)?
        }
        Err(e) => return Err(e),
    };
    parse_sha(&String::from_utf8_lossy(&out)).ok_or(Error::Tool {
        tool: "sha256",
        message: "checksum tool produced no digest".to_owned(),
    })
}
