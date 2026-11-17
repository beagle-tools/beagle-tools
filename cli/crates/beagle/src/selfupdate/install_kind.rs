//! How the running `beagle` got onto this machine, which decides what
//! "update" means for it.

use std::path::Path;

/// Set by release CI (`_release-cli.yml`) to the target triple it built
/// for; absent from any other build. `option_env!` makes cargo rebuild
/// when it changes.
const RELEASE_TARGET: Option<&str> = option_env!("BEAGLE_RELEASE_TARGET");

/// The Homebrew formula that installs this binary.
pub const BREW_FORMULA: &str = "beagle-tools/tap/beagle";

/// How this copy of `beagle` was installed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    /// A release binary that Homebrew manages: `brew upgrade` owns it.
    Homebrew,
    /// A release tarball unpacked by hand: swapped in place.
    Release,
    /// Built from source (`cargo install`, `cargo run`): rebuild it.
    Source,
}

impl InstallKind {
    /// Classifies the binary at `exe` (resolved through symlinks, so a
    /// Homebrew `bin/beagle` link lands in its Cellar). Release binaries
    /// carry [`RELEASE_TARGET`]; anything else is a source build.
    pub fn detect(exe: &Path) -> Self {
        // An unresolvable path (deleted mid-run, say) is classified as
        // given: at worst a Homebrew symlink reads as a tarball install.
        let resolved = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
        Self::classify(&resolved, RELEASE_TARGET.is_some())
    }

    /// [`Self::detect`] without the filesystem, for tests.
    pub fn classify(resolved_exe: &Path, release_build: bool) -> Self {
        if !release_build {
            return Self::Source;
        }
        let in_cellar = resolved_exe
            .components()
            .any(|c| c.as_os_str() == "Cellar" || c.as_os_str() == ".linuxbrew");
        if in_cellar {
            Self::Homebrew
        } else {
            Self::Release
        }
    }

    /// How `beagle version` describes this build.
    pub fn describe(self) -> String {
        match (self, RELEASE_TARGET) {
            (Self::Homebrew, Some(target)) => format!("{target}, Homebrew"),
            (Self::Release, Some(target)) => target.to_owned(),
            (Self::Release | Self::Homebrew, None) | (Self::Source, _) => "source build".to_owned(),
        }
    }
}

/// What to run to update a source build.
pub fn source_update_hint(repo_url: &str) -> String {
    format!(
        "this beagle was built from source, so update it the same way:\n\
         \x20 from a checkout:  git pull && cargo install --locked --path cli/crates/beagle\n\
         \x20 or straight from GitHub:  cargo install --locked --force --git {repo_url} beagle-cli\n\
         (or switch to a release build: brew install {BREW_FORMULA})"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_builds_are_source_wherever_they_live() {
        let cellar = Path::new("/opt/homebrew/Cellar/beagle/0.1.0/bin/beagle");
        assert_eq!(InstallKind::classify(cellar, false), InstallKind::Source);
        let cargo = Path::new("/home/me/.cargo/bin/beagle");
        assert_eq!(InstallKind::classify(cargo, false), InstallKind::Source);
    }

    #[test]
    fn release_builds_split_into_homebrew_and_tarball() {
        let cellar = Path::new("/opt/homebrew/Cellar/beagle/0.1.0/bin/beagle");
        assert_eq!(InstallKind::classify(cellar, true), InstallKind::Homebrew);
        let linuxbrew = Path::new("/home/linuxbrew/.linuxbrew/Cellar/beagle/0.1.0/bin/beagle");
        assert_eq!(
            InstallKind::classify(linuxbrew, true),
            InstallKind::Homebrew
        );
        let local = Path::new("/usr/local/bin/beagle");
        assert_eq!(InstallKind::classify(local, true), InstallKind::Release);
    }

    #[test]
    fn the_source_hint_names_both_cargo_routes() {
        let hint = source_update_hint("https://github.com/acme/widgets");
        assert!(hint.contains("--path cli/crates/beagle"), "{hint}");
        assert!(
            hint.contains("--git https://github.com/acme/widgets beagle-cli"),
            "{hint}"
        );
    }
}
