//! The download → verify → extract → swap pipeline.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use super::{Error, Result};
use crate::process::output_with_timeout;

/// curl's `--connect-timeout` for every request, in seconds.
const CONNECT_TIMEOUT_SECS: u64 = 10;
/// curl's `--max-time` for a tarball or checksum download, in seconds.
const DOWNLOAD_MAX_SECS: u64 = 600;
/// curl's `--max-time` for the release listing, in seconds.
pub(super) const LISTING_MAX_SECS: u64 = 30;
/// Bound on the local tools (`tar`, `shasum`), generous for a big tarball.
const LOCAL_TOOL_TIMEOUT: Duration = Duration::from_secs(120);

/// Distinguishes concurrent installs (tests run in parallel in one process).
static INSTALL_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Downloads a release tarball and its checksum from the given URLs,
/// verifies the sha256, extracts it, and atomically swaps `binary` from the
/// tarball root over `exe`. URL-based so tests drive the whole flow with
/// `file://` URLs.
///
/// Each of `companions` (e.g. a daemon shipped beside the CLI) is swapped
/// too, but only when it is in the tarball AND already installed next to
/// `exe` — an update never adds binaries the user didn't install.
///
/// # Errors
/// [`Error::Tool`] / [`Error::ChecksumMismatch`] / [`Error::Io`] for
/// download, verification and install failures. `exe` is untouched on any
/// failure before the final rename.
pub fn install(
    tarball_url: &str,
    sha_url: &str,
    exe: &Path,
    binary: &str,
    companions: &[&str],
) -> Result<()> {
    let workdir = std::env::temp_dir().join(format!(
        "beagle-update-{}-{}",
        std::process::id(),
        INSTALL_SEQ.fetch_add(1, Ordering::Relaxed),
    ));
    fs::create_dir_all(&workdir).map_err(|e| Error::io(&workdir, e))?;
    let result = install_in(&workdir, tarball_url, sha_url, exe, binary, companions);
    // Best effort: it is a temp dir, and the install result matters more.
    let _ = fs::remove_dir_all(&workdir);
    result
}

fn install_in(
    workdir: &Path,
    tarball_url: &str,
    sha_url: &str,
    exe: &Path,
    binary: &str,
    companions: &[&str],
) -> Result<()> {
    let tarball = workdir.join("release.tar.gz");
    download(tarball_url, &tarball)?;
    let sha_file = workdir.join("release.tar.gz.sha256");
    download(sha_url, &sha_file)?;

    let expected = read_expected_sha(&sha_file)?;
    let actual = sha256_of(&tarball)?;
    if expected != actual {
        return Err(Error::ChecksumMismatch {
            path: tarball,
            expected,
            actual,
        });
    }

    let extracted = workdir.join("extracted");
    fs::create_dir_all(&extracted).map_err(|e| Error::io(&extracted, e))?;
    run_tool(
        "tar",
        &[
            "-xzf",
            &tarball.to_string_lossy(),
            "-C",
            &extracted.to_string_lossy(),
        ],
    )?;
    let new_binary = extracted.join(binary);
    if !new_binary.is_file() {
        return Err(Error::Tool {
            tool: "tar",
            message: format!("extracted archive has no `{binary}` binary at its root"),
        });
    }
    swap_in(&new_binary, exe)?;

    let dir = exe.parent().unwrap_or_else(|| Path::new("."));
    for companion in companions {
        let new_file = extracted.join(companion);
        let installed = dir.join(companion);
        if new_file.is_file() && installed.is_file() {
            swap_in(&new_file, &installed)?;
        }
    }
    Ok(())
}

/// Replaces `dest` with `new_file`: staged next to the destination so the
/// final rename is atomic (same filesystem); `fs::copy` carries the
/// executable bit over from the extracted file.
fn swap_in(new_file: &Path, dest: &Path) -> Result<()> {
    let name = dest
        .file_name()
        .map_or_else(|| "binary".into(), |n| n.to_string_lossy().into_owned());
    let staging = dest
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(".{name}.update"));
    fs::copy(new_file, &staging).map_err(|e| Error::io(&staging, e))?;
    fs::rename(&staging, dest).map_err(|e| {
        // Best effort: the rename failure is the error worth reporting.
        let _ = fs::remove_file(&staging);
        Error::io(dest, e)
    })?;
    Ok(())
}

/// Downloads `url` to `to` via curl. `file://` URLs work too.
fn download(url: &str, to: &Path) -> Result<()> {
    curl_stdout(
        &["-fsSL", "--retry", "2", "-o", &to.to_string_lossy(), url],
        DOWNLOAD_MAX_SECS,
    )
    .map(|_| ())
}

/// Runs curl with `args` and returns its stdout. Every request gets
/// `--connect-timeout 10 --max-time <max_secs>`, and the process itself is
/// bounded a little past that in case curl overruns its own limit.
pub(super) fn curl_stdout(args: &[&str], max_secs: u64) -> Result<String> {
    let connect = CONNECT_TIMEOUT_SECS.to_string();
    let max = max_secs.to_string();
    let mut cmd = Command::new("curl");
    cmd.args(["--connect-timeout", &connect, "--max-time", &max])
        .args(args);
    let bound = Duration::from_secs(max_secs + 15);
    let output = output_with_timeout(&mut cmd, bound).map_err(|e| {
        let message = if e.kind() == std::io::ErrorKind::NotFound {
            "curl not found on PATH; install curl to update".to_owned()
        } else if e.kind() == std::io::ErrorKind::TimedOut {
            format!("{e} — is GitHub reachable?")
        } else {
            e.to_string()
        };
        Error::Tool {
            tool: "curl",
            message,
        }
    })?;
    if !output.status.success() {
        return Err(Error::Tool {
            tool: "curl",
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Runs a local tool (`tar`, `shasum`) bounded by [`LOCAL_TOOL_TIMEOUT`].
fn local_tool(tool: &str, args: &[&str]) -> std::io::Result<Output> {
    output_with_timeout(Command::new(tool).args(args), LOCAL_TOOL_TIMEOUT)
}

fn run_tool(tool: &'static str, args: &[&str]) -> Result<()> {
    let output = local_tool(tool, args).map_err(|e| Error::Tool {
        tool,
        message: e.to_string(),
    })?;
    if !output.status.success() {
        return Err(Error::Tool {
            tool,
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(())
}

/// Reads the expected checksum from a published `.sha256` file (format:
/// `<hex>  <filename>`).
fn read_expected_sha(path: &Path) -> Result<String> {
    let raw = fs::read_to_string(path).map_err(|e| Error::io(path, e))?;
    parse_sha(&raw).ok_or_else(|| Error::Tool {
        tool: "sha256",
        message: format!("`{}` does not contain a sha256 hex digest", path.display()),
    })
}

/// Extracts the leading 64-hex-char digest from a checksum line, lowercased.
fn parse_sha(line: &str) -> Option<String> {
    let token = line.split_whitespace().next()?;
    (token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| token.to_ascii_lowercase())
}

/// Computes a file's sha256 via `shasum -a 256` (macOS) or `sha256sum`
/// (Linux), whichever exists.
fn sha256_of(path: &Path) -> Result<String> {
    let path_str = path.to_string_lossy();
    let attempts: [(&str, Vec<&str>); 2] = [
        ("shasum", vec!["-a", "256", &path_str]),
        ("sha256sum", vec![&path_str]),
    ];
    for (tool, args) in attempts {
        match local_tool(tool, &args) {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return parse_sha(&stdout).ok_or(Error::Tool {
                    tool: "sha256",
                    message: "checksum tool produced no digest".to_owned(),
                });
            }
            Ok(output) => {
                return Err(Error::Tool {
                    tool: "sha256",
                    message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(Error::Tool {
                    tool: "sha256",
                    message: e.to_string(),
                });
            }
        }
    }
    Err(Error::Tool {
        tool: "sha256",
        message: "neither shasum nor sha256sum found on PATH".to_owned(),
    })
}

#[cfg(test)]
#[path = "tests/install.rs"]
mod tests;
