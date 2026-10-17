//! End to end: install a fake extension from a `file://` release server
//! through the public `run` entry point, dispatch to it, update, roll back
//! offline; then drive the real `beagle` binary to check exec forwarding.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic, missing_docs)] // panicking is the correct failure mode in tests

use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use beagle_cli::paths::Layout;
use beagle_cli::registry::ExtensionSpec;
use beagle_cli::releases::ReleaseSource;
use beagle_cli::{run, Context, Error, Outcome};

const TARGET: &str = "test-target";
const FAKE: &[ExtensionSpec] = &[ExtensionSpec {
    name: "fake",
    aliases: &["phony"],
    release_app: "fake",
    tag_prefix: "beagle-fake-v",
    asset_prefix: "beagle-fake-ext",
    targets: &[TARGET],
    description: "a fake extension",
}];

/// A tempdir holding the data dir and a release server.
struct World {
    tmp: tempfile::TempDir,
    releases: Vec<String>,
}

impl World {
    fn new() -> Self {
        Self {
            tmp: tempfile::tempdir().unwrap(),
            releases: Vec::new(),
        }
    }

    fn server(&self) -> PathBuf {
        self.tmp.path().join("server")
    }

    /// Publishes `beagle-fake` `version`: a script echoing its args and env.
    fn publish(&mut self, version: &str) {
        let tag = format!("beagle-fake-v{version}");
        let asset = format!("beagle-fake-ext-{TARGET}.tar.gz");
        let pkg = self.tmp.path().join(format!("pkg-{version}"));
        fs::create_dir_all(&pkg).unwrap();
        fs::write(
            pkg.join("extension.toml"),
            format!(
                "schema = 1\nname = \"fake\"\nversion = \"{version}\"\nprotocol = 1\n\
                 min_base_version = \"0.0.0\"\nbin = \"beagle-fake\"\ndescription = \"fake ext\"\n"
            ),
        )
        .unwrap();
        let script = pkg.join("beagle-fake");
        fs::write(
            &script,
            format!(
                "#!/bin/sh\necho \"fake {version}\"\nfor a in \"$@\"; do echo \"arg=[$a]\"; done\n\
                 echo \"ext=$BEAGLE_EXTENSION ver=$BEAGLE_EXTENSION_VERSION as=$BEAGLE_INVOKED_AS\"\n"
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

        let dir = self.server().join(&tag);
        fs::create_dir_all(&dir).unwrap();
        let tarball = dir.join(&asset);
        let tar = Command::new("tar")
            .arg("-czf")
            .arg(&tarball)
            .arg("-C")
            .arg(&pkg)
            .args(["extension.toml", "beagle-fake"])
            .status()
            .unwrap();
        assert!(tar.success());
        let digest = beagle_cli::install::sha256_of(&tarball).unwrap();
        fs::write(
            dir.join(format!("{asset}.sha256")),
            format!("{digest}  {asset}\n"),
        )
        .unwrap();
        self.releases.push(format!(
            r#"{{"tag_name": "{tag}", "assets": [{{"name": "{asset}"}}]}}"#
        ));
        fs::write(
            self.server().join("releases.json"),
            format!("[{}]", self.releases.join(",")),
        )
        .unwrap();
    }

    fn ctx(&self) -> Context<'static> {
        let server = self.server();
        Context {
            registry: FAKE,
            layout: Layout::new(self.tmp.path().join("data/beagle")),
            source: ReleaseSource {
                releases_url: format!("file://{}", server.join("releases.json").display()),
                download_base: format!("file://{}", server.display()),
            },
            target: Some(TARGET.to_owned()),
            path_var: OsString::new(),
            exe: PathBuf::from("/usr/local/bin/beagle"),
            home: None,
            interactive: false,
            notices: false,
            lock_wait: Duration::ZERO,
        }
    }
}

/// Runs `beagle args…`; returns the outcome and stdout.
fn run_cli(ctx: &Context<'_>, args: &[&str]) -> (Result<Outcome, Error>, String) {
    let mut out = Vec::new();
    let outcome = run(args.iter().map(OsString::from).collect(), ctx, &mut out);
    (outcome, String::from_utf8(out).unwrap())
}

/// Runs `beagle args…` expecting a dispatch; returns the extension's stdout.
fn dispatch(ctx: &Context<'_>, args: &[&str]) -> String {
    let (outcome, _) = run_cli(ctx, args);
    let Ok(Outcome::Exec(mut plan)) = outcome else {
        panic!("{args:?} did not dispatch: {outcome:?}");
    };
    // Spawn rather than exec, so the test process survives.
    let output = plan.command.output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn exit_code(outcome: &Result<Outcome, Error>) -> u8 {
    match outcome {
        Ok(Outcome::Exit(code)) => *code,
        Ok(Outcome::Exec(_)) => panic!("unexpected exec"),
        Err(e) => e.exit_code(),
    }
}

#[test]
fn install_dispatch_update_and_offline_rollback() {
    let mut world = World::new();
    world.publish("0.28.0");
    world.publish("0.29.0");
    let ctx = world.ctx();

    let (outcome, out) = run_cli(
        &ctx,
        &[
            "extensions",
            "install",
            "fake",
            "--version",
            "0.29.0",
            "--yes",
        ],
    );
    assert_eq!(exit_code(&outcome), 0, "{outcome:?}");
    assert_eq!(out.trim(), "installed fake 0.29.0");
    let ext_dir = ctx.layout.ext_dir("fake");
    assert!(ext_dir.join("0.29.0/bin/beagle-fake").is_file());
    assert_eq!(
        fs::read_link(ext_dir.join("current")).unwrap(),
        Path::new("0.29.0")
    );

    let out = dispatch(&ctx, &["fake", "a", "--", "", "b c"]);
    assert_eq!(
        out,
        "fake 0.29.0\narg=[a]\narg=[--]\narg=[]\narg=[b c]\next=fake ver=0.29.0 as=beagle fake\n"
    );
    // Aliases dispatch to the canonical extension.
    assert!(dispatch(&ctx, &["phony", "--version"]).starts_with("fake 0.29.0\narg=[--version]"));

    let (outcome, out) = run_cli(&ctx, &["fake", "update", "--version", "0.28.0"]);
    assert_eq!(exit_code(&outcome), 0, "{outcome:?}");
    assert_eq!(out.trim(), "fake: 0.29.0 → 0.28.0");
    assert!(dispatch(&ctx, &["fake"]).starts_with("fake 0.28.0"));

    // Back to 0.29.0 with the release server gone: proves `use` is offline.
    fs::remove_dir_all(world.server()).unwrap();
    let (outcome, out) = run_cli(&ctx, &["extensions", "use", "fake", "v0.29.0"]);
    assert_eq!(exit_code(&outcome), 0, "{outcome:?}");
    assert_eq!(out.trim(), "fake 0.29.0 is now current");
    assert!(dispatch(&ctx, &["fake"]).starts_with("fake 0.29.0"));

    let (_, out) = run_cli(&ctx, &["extensions", "path", "fake"]);
    assert_eq!(
        out.trim(),
        ext_dir
            .join("current/bin/beagle-fake")
            .display()
            .to_string()
    );
    let (_, out) = run_cli(&ctx, &["--version"]);
    assert!(out.contains("  fake 0.29.0"), "{out}");
}

#[test]
fn non_interactive_install_takes_the_latest_without_prompting() {
    let mut world = World::new();
    world.publish("0.1.0");
    world.publish("0.2.0");
    let ctx = world.ctx();
    let (outcome, out) = run_cli(&ctx, &["ext", "install", "fake"]);
    assert_eq!(exit_code(&outcome), 0, "{outcome:?}");
    assert_eq!(out.trim(), "installed fake 0.2.0");
    let (_, out) = run_cli(&ctx, &["ext", "update", "--all"]);
    assert_eq!(out.trim(), "fake is up to date (0.2.0)");
}

#[test]
fn a_corrupt_download_aborts_and_keeps_current() {
    let mut world = World::new();
    world.publish("0.1.0");
    let ctx = world.ctx();
    run_cli(&ctx, &["ext", "install", "fake", "--yes"])
        .0
        .unwrap();
    world.publish("0.2.0");
    let tarball = world.server().join(format!(
        "beagle-fake-v0.2.0/beagle-fake-ext-{TARGET}.tar.gz"
    ));
    fs::write(&tarball, b"corrupted").unwrap();

    let (outcome, _) = run_cli(&ctx, &["fake", "update"]);
    let err = outcome.unwrap_err();
    assert!(
        err.to_string()
            .starts_with("checksum mismatch for beagle-fake-ext-"),
        "{err}"
    );
    assert!(dispatch(&ctx, &["fake"]).starts_with("fake 0.1.0"));
}

#[test]
fn errors_name_the_fix() {
    let world = World::new();
    let ctx = world.ctx();
    let (outcome, _) = run_cli(&ctx, &["nope"]);
    assert_eq!(exit_code(&outcome), 2);
    assert_eq!(outcome.unwrap_err().to_string(), "unknown command \"nope\"");
    let (outcome, _) = run_cli(&ctx, &["fak"]);
    assert_eq!(
        outcome.unwrap_err().to_string(),
        "unknown command \"fak\" — did you mean \"fake\"?"
    );

    let (outcome, _) = run_cli(&ctx, &["fake"]);
    assert_eq!(exit_code(&outcome), 1);
    assert_eq!(
        outcome.unwrap_err().to_string(),
        "\"fake\" is not installed — run: beagle extensions install fake"
    );

    let (outcome, _) = run_cli(&ctx, &["extensions", "install", "extensions"]);
    assert_eq!(
        outcome.unwrap_err().to_string(),
        "\"extensions\" is a reserved command name, not an extension"
    );
}

/// The real binary: `beagle-<name>` on PATH gets argv byte-for-byte via
/// exec, including `--`, empty strings and non-UTF-8 bytes.
#[test]
fn the_binary_execs_path_extensions_with_exact_args() {
    let tmp = tempfile::tempdir().unwrap();
    let bin = tmp.path().join("bin");
    fs::create_dir_all(&bin).unwrap();
    let script = bin.join("beagle-devtool");
    fs::write(
        &script,
        "#!/bin/sh\nfor a in \"$@\"; do printf '%s\\0' \"$a\"; done\nexit 7\n",
    )
    .unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();

    let weird = OsString::from_vec(vec![b'x', 0xff, b'y']);
    let output = Command::new(env!("CARGO_BIN_EXE_beagle"))
        .arg("devtool")
        .args(["--", "", "-h"])
        .arg(&weird)
        .env("PATH", &bin)
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(7), "exit code passes through");
    assert_eq!(output.stdout, b"--\0\0-h\0x\xffy\0");

    let unknown = Command::new(env!("CARGO_BIN_EXE_beagle"))
        .arg("nope")
        .env("PATH", &bin)
        .env("HOME", tmp.path())
        .output()
        .unwrap();
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unknown command \"nope\""));

    let missing = Command::new(env!("CARGO_BIN_EXE_beagle"))
        .arg("rca")
        .env("PATH", &bin)
        .env("HOME", tmp.path())
        .env("XDG_DATA_HOME", tmp.path().join("data"))
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("beagle extensions install rca"));
}
