//! A fake release server on disk for unit tests: tarballs, checksums and a
//! releases listing reachable through `file://` URLs.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)] // panicking is the correct failure mode in tests

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::install::sha256_of;
use crate::registry::ExtensionSpec;
use crate::releases::{Release, ReleaseSource};
use crate::version::Version;

/// The target every fixture publishes for.
pub const TARGET: &str = "test-target";

/// A test-only extension spec, published for [`TARGET`].
pub const FAKE: ExtensionSpec = ExtensionSpec {
    name: "fake",
    aliases: &[],
    release_app: "fake",
    tag_prefix: "beagle-fake-v",
    asset_prefix: "beagle-fake-ext",
    targets: &[TARGET],
    description: "a fake extension",
};

/// A release server rooted in a tempdir.
pub struct Server {
    root: PathBuf,
    entries: Vec<String>,
}

impl Server {
    pub fn new(root: &Path) -> Self {
        let root = root.join("server");
        fs::create_dir_all(&root).unwrap();
        Self {
            root,
            entries: Vec::new(),
        }
    }

    pub fn source(&self) -> ReleaseSource {
        ReleaseSource {
            releases_url: format!("file://{}", self.root.join("releases.json").display()),
            download_base: format!("file://{}", self.root.display()),
        }
    }

    /// Publishes `version` of [`FAKE`] with a well-formed tarball whose
    /// `beagle-fake` script prints its version.
    pub fn publish(&mut self, version: &str) -> Release {
        let manifest = manifest(version);
        let script = format!("#!/bin/sh\necho fake {version} \"$@\"\n");
        self.publish_raw(
            version,
            &[
                ("extension.toml", manifest.as_bytes(), false),
                ("beagle-fake", script.as_bytes(), true),
            ],
        )
    }

    /// Publishes `version` with exactly `files` (name, bytes, executable).
    pub fn publish_raw(&mut self, version: &str, files: &[(&str, &[u8], bool)]) -> Release {
        let tag = format!("{}{version}", FAKE.tag_prefix);
        let asset = FAKE.asset_name(TARGET);
        let pkg = self.root.join(format!("pkg-{version}"));
        fs::create_dir_all(&pkg).unwrap();
        for (name, bytes, exec) in files {
            let path = pkg.join(name);
            fs::write(&path, bytes).unwrap();
            if *exec {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        let dir = self.root.join(&tag);
        fs::create_dir_all(&dir).unwrap();
        let tarball = dir.join(&asset);
        let names: Vec<&str> = files.iter().map(|f| f.0).collect();
        let ok = Command::new("tar")
            .arg("-czf")
            .arg(&tarball)
            .arg("-C")
            .arg(&pkg)
            .args(&names)
            .status()
            .unwrap()
            .success();
        assert!(ok, "tar failed");
        let digest = sha256_of(&tarball).unwrap();
        fs::write(
            dir.join(format!("{asset}.sha256")),
            format!("{digest}  {asset}\n"),
        )
        .unwrap();
        self.entries.push(format!(
            r#"{{"tag_name": "{tag}", "assets": [{{"name": "{asset}"}}, {{"name": "{asset}.sha256"}}]}}"#
        ));
        fs::write(
            self.root.join("releases.json"),
            format!("[{}]", self.entries.join(",")),
        )
        .unwrap();
        Release {
            tag,
            version: version.parse().unwrap(),
            prerelease: false,
            assets: vec![asset],
        }
    }

    /// The published tarball for `version`, to corrupt in tests.
    pub fn tarball(&self, version: &str) -> PathBuf {
        self.root
            .join(format!("{}{version}", FAKE.tag_prefix))
            .join(FAKE.asset_name(TARGET))
    }
}

/// A valid `extension.toml` for [`FAKE`] at `version`.
pub fn manifest(version: &str) -> String {
    format!(
        "schema = 1\nname = \"fake\"\nversion = \"{version}\"\nprotocol = 1\n\
         min_base_version = \"0.0.0\"\nbin = \"beagle-fake\"\ndescription = \"a fake extension\"\n"
    )
}

pub fn v(s: &str) -> Version {
    s.parse().unwrap()
}
