//! The closed, compiled-in set of extensions the base can install.
//!
//! There is no remote index: every entry points at this repo's own GitHub
//! releases, so there is nothing to trust beyond them. The table must stay in
//! sync with `scripts/release/apps.sh` (tag and asset prefixes); a test parses
//! that script and fails on drift.

/// Names the base handles itself; none may ever name an extension.
pub const RESERVED: &[&str] = &[
    "extensions",
    "ext",
    "update",
    "version",
    "help",
    "doctor",
    "completions",
    "self",
];

/// One installable extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtensionSpec {
    /// Canonical name: `beagle <name>` runs binary `beagle-<name>`.
    pub name: &'static str,
    /// Old names that still resolve to this extension (for renames).
    pub aliases: &'static [&'static str],
    /// The release app in `scripts/release/apps.sh` that ships it.
    pub release_app: &'static str,
    /// Release tag prefix, e.g. `beagle-rca-v`.
    pub tag_prefix: &'static str,
    /// Tarball name prefix: assets are `<asset_prefix>-<target>.tar.gz`.
    pub asset_prefix: &'static str,
    /// Target triples a build is published for.
    pub targets: &'static [&'static str],
    /// One line for `beagle extensions available`.
    pub description: &'static str,
}

impl ExtensionSpec {
    /// The tarball asset name for `target`.
    pub fn asset_name(&self, target: &str) -> String {
        format!("{}-{target}.tar.gz", self.asset_prefix)
    }

    /// Whether a build is published for `target`.
    pub fn supports(&self, target: &str) -> bool {
        self.targets.contains(&target)
    }
}

const ALL_TARGETS: &[&str] = &[
    "aarch64-apple-darwin",
    "x86_64-apple-darwin",
    "x86_64-unknown-linux-musl",
];
const MACOS_TARGETS: &[&str] = &["aarch64-apple-darwin", "x86_64-apple-darwin"];

/// Every extension the base knows about.
pub const REGISTRY: &[ExtensionSpec] = &[
    ExtensionSpec {
        name: "rca",
        aliases: &[],
        release_app: "rca",
        tag_prefix: "beagle-rca-v",
        asset_prefix: "beagle-rca-ext",
        targets: ALL_TARGETS,
        description: "AI-first incident debugging workspaces",
    },
    ExtensionSpec {
        name: "agentd",
        aliases: &[],
        release_app: "workflows",
        tag_prefix: "beagle-workflows-v",
        asset_prefix: "beagle-workflows-ext",
        targets: MACOS_TARGETS,
        description: "Scheduled and on-demand coding-agent jobs",
    },
    ExtensionSpec {
        name: "tandem",
        aliases: &[],
        release_app: "tandem",
        tag_prefix: "beagle-tandem-v",
        asset_prefix: "beagle-tandem-ext",
        targets: ALL_TARGETS,
        description: "Review GitHub PRs alongside coding agents",
    },
];

/// The base's own release line, shaped like an extension so the same
/// listing and install code serves `beagle update`.
pub const BASE: ExtensionSpec = ExtensionSpec {
    name: "beagle",
    aliases: &[],
    release_app: "cli",
    tag_prefix: "beagle-cli-v",
    asset_prefix: "beagle-cli",
    targets: ALL_TARGETS,
    description: "The beagle base CLI",
};

/// Looks `name` up in `registry`, resolving aliases.
pub fn find<'r>(registry: &'r [ExtensionSpec], name: &str) -> Option<&'r ExtensionSpec> {
    registry
        .iter()
        .find(|spec| spec.name == name || spec.aliases.contains(&name))
}

/// Whether `name` is a base command (and so never an extension).
pub fn is_reserved(name: &str) -> bool {
    RESERVED.contains(&name)
}

/// The compile-time target triple, named exactly as release assets are.
/// `None` on platforms nothing is built for.
pub fn release_target() -> Option<&'static str> {
    if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        Some("aarch64-apple-darwin")
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        Some("x86_64-apple-darwin")
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        Some("x86_64-unknown-linux-musl")
    } else {
        None
    }
}

#[cfg(test)]
#[path = "tests/registry.rs"]
mod tests;
