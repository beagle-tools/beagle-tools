# beagle-tools

Developer tools, in one monorepo:

- **[tandem](tandem/)**: a desktop app for reviewing GitHub PRs alongside coding agents.
- **[rca](rca/)**: Beagle, a set of RCA (root-cause analysis) workspaces with a TUI/CLI, an agent daemon, a desktop app and a static site.
- **[workflows](workflows/)**: agentd, a scheduled agent-job daemon with CLIs, a TUI and the Agentdesk desktop app.
- **[utils](utils/)**: `beagle-utils`, Rust helpers shared across the apps.

Every app follows the same [coding standards](docs/CODING_STANDARDS.md) and
passes the same CI gate. Run `make check` (or `make check-<app>`) from the
repo root to run exactly what CI runs. Agents should start with
[AGENTS.md](AGENTS.md).
