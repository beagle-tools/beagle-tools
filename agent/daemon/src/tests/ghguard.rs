//! Tests for the gh shim: the installed shim in front of a fake `gh` that
//! records what reached it, exactly as a run sees them.

// Tests should fail loudly on a broken fixture; unwrapping is the clearest way.
#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{write_exec, GH_SHIM_SH};

/// `shim/gh` (the guard) and `real/gh` (a fake that prints its args).
fn setup(tag: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("agentd-ghguard-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    for dir in ["shim", "real"] {
        std::fs::create_dir_all(root.join(dir)).unwrap();
    }
    write_exec(&root.join("shim/gh"), GH_SHIM_SH).unwrap();
    write_exec(&root.join("real/gh"), "#!/bin/sh\necho \"REAL gh $*\"\n").unwrap();
    root
}

/// Runs `gh <args>` with the shim first on PATH: (succeeded, stdout, stderr).
fn gh(root: &Path, args: &str) -> (bool, String, String) {
    let path = format!(
        "{}:{}:{}",
        root.join("shim").display(),
        root.join("real").display(),
        std::env::var("PATH").unwrap()
    );
    let out = Command::new("sh")
        .arg("-c")
        .arg(format!("gh {args}"))
        .env("PATH", path)
        .output()
        .unwrap();
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

#[test]
fn writes_are_blocked_with_the_guard_message() {
    let root = setup("block");
    for args in [
        "pr merge 1",
        "pr merge --squash 1",
        "-R acme/widgets pr merge 1",
        "pr review 1 --approve",
        "pr review -a 1",
        "api -X POST repos/acme/widgets/issues",
        "api --method=patch repos/acme/widgets/pulls/1",
        "api -XDELETE repos/acme/widgets/git/refs/heads/x",
        "api repos/acme/widgets/issues -f title=x",
        "api repos/acme/widgets/issues --input body.json",
        "api graphql -f query='mutation { addStar(input: {}) { clientMutationId } }'",
    ] {
        let (ok, out, err) = gh(&root, args);
        assert!(!ok, "{args}: should be blocked");
        assert!(err.contains("agentd: BLOCKED gh"), "{args}: {err}");
        assert!(err.contains("allow_github_writes"), "{args}: {err}");
        assert!(!out.contains("REAL gh"), "{args} reached the real gh");
    }
}

#[test]
fn reads_reach_the_real_gh() {
    let root = setup("allow");
    for args in [
        "pr view 1",
        "pr list --state open",
        "pr review 1 --comment -b looks-good",
        "pr checks 1",
        "api repos/acme/widgets/pulls",
        "api -X GET repos/acme/widgets/pulls -f state=open",
        "api graphql -f query='query { viewer { login } }'",
        "auth status",
    ] {
        let (ok, out, err) = gh(&root, args);
        assert!(ok, "{args}: {err}");
        assert!(out.starts_with("REAL gh "), "{args}: {out}");
    }
}
