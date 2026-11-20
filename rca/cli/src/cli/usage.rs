//! The `beagle rca --help` text.

/// The `--help` text. Written for the unified `beagle` command
/// (`beagle rca …`); the standalone rca binary accepts the same spelling
/// (`crate::run_standalone` drops a leading `rca`), so it is right there too.
pub const USAGE: &str = "\
beagle rca — a TUI for root-cause analysis workspaces

USAGE:
    beagle rca [--root <dir>]             open the TUI (default)
    beagle rca new <id> --title <title>   scaffold a new RCA workspace
                  [--severity <sev>]        critical|high|medium|low|info (default: medium)
                  [--system <name>]...      systems involved (repeatable)
                  [--root <dir>]
    beagle rca list [--status <status>]   print workspaces to stdout,
                  [--severity <sev>]        optionally filtered
                  [--archived]              include archived workspaces
                  [--json]                  emit a JSON array instead of
                                              the text table (for scripts)
                  [--root <dir>]
    beagle rca archive <id> [--force]     move a finished RCA to
                  [--root <dir>]            rcas/archive/ (kept, but out of
                                            the sidebar; --force skips the
                                            finished-status check)
    beagle rca unarchive <id>             move an archived RCA back to the
                  [--root <dir>]            active list
    beagle rca publish <id>               mark an RCA public (the web app
                  [--root <dir>]            builds only published RCAs)
    beagle rca unpublish <id>             make an RCA private again
                  [--root <dir>]
    beagle rca handoff <id>               launch the configured agent on a
                  [--root <dir>]            reviewed RCA ([handoff] in config)
    beagle rca status <id> <status>       set a workspace's status; a running
                  [--root <dir>]            TUI picks the change up live
                                            (investigating|fixing|review|agent|final-review|finished)
    beagle rca log <id> <message...>      append a timestamped bullet to the
                  [--root <dir>]            workspace's log.md (the Log tab)
    beagle rca pr add <id> <url>          attach a remediation PR to the RCA;
                  [--root <dir>]            the TUI tracks merge status via gh
    beagle rca pr list <id>               print attached PRs (live state
                  [--root <dir>]            included when gh is available)
    beagle rca ticket add <id> <ref>      attach a Linear/GitHub issue
                  [--root <dir>]            (URL or ABC-123 id)
    beagle rca ticket list <id>           print attached tickets
                  [--root <dir>]
    beagle rca ticket sync [<id>]         poll tickets, attach their PRs
                  [--root <dir>]            (omit <id> to sync all)
    beagle rca similar <id>               print past RCAs related to this one
                  [--root <dir>]            (shared systems and tags, ranked)
    beagle rca export <id>                export one RCA as a single markdown
                  [--out <file>]            document (default: exports/<id>.md)
                  [--root <dir>]
    beagle rca context <id>               print the review bundle (writeup +
                  [--root <dir>]            toolbox) to stdout (/beagle-review)
    beagle rca init [--root <dir>]        set up a store: interactively asks
                                            for the root and writes a .beagle
                                            pinning it (skipped with --root or
                                            when piped), then scaffolds
                                            toolbox.md + systems/ templates
    beagle rca config                     edit the config file and validate it
    beagle rca skill [status|install]     show or install the /beagle skills
    beagle rca agent <action>             manage the beagle-agentd daemon:
                                            install|uninstall|start|stop|status
    beagle rca version                    print rca's version
    beagle rca banner                     print the BEAGLE banner
    beagle rca --help | --version

To update rca, update beagle: `beagle update` (every tool ships in the one
binary).

The <id> is a lowercase slug ([a-z0-9-], max 64 chars) and becomes the
directory name under <root>/rcas/. `--root <dir>` (or `--root=<dir>`) may
come before or after the command. Without it, the config file's `root` is
used, then the current directory.";
