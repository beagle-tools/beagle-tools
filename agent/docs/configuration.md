# Configuration and safety rails

Everything you can tune that is not a manifest: where agentd keeps things,
its two config files, the daemon's flags, and the guards every run gets.
Manifest fields are in [manifests.md](manifests.md).

## Where things live

| What | Default | Moved by |
|---|---|---|
| Manifests | `~/.config/agentd/manifests/` | `$AGENTD_MANIFESTS_DIR`, or `apply --manifests DIR` / `daemon run --manifests DIR` |
| Config (`ui.yaml`, `agentd.yaml`) | `~/.config/agentd/` | `$AGENTD_CONFIG_DIR`, else `$XDG_CONFIG_HOME/agentd` |
| State: `state.db`, `logs/`, `data/<run id>/`, `worktrees/`, `summaries/`, `agentd.heartbeat`, `lib/` (the guards) | `~/.local/state/agentd/` | `$AGENTD_STATE_DIR`, else `$XDG_STATE_HOME/agentd` |
| Daemon logs | `~/Library/Logs/agentd/agentd.{out,err}.log` | — |
| launchd unit | `~/Library/LaunchAgents/com.agentd.daemon.plist` | — |

The daemon, the CLI, the dashboard and Agentdesk all read the same
`state.db`, so they can never disagree. Only the daemon changes a run's
state; the others only queue runs and mark runs killed.

Moving `$AGENTD_MANIFESTS_DIR` does not move the config directory.

## `ui.yaml` and `agentd.yaml`

Both are optional, and unknown keys are ignored, so a newer file still
loads on an older install.

```yaml
# ~/.config/agentd/ui.yaml — how the dashboard and Agentdesk show things
ticket_url: https://tracker.example.com/issue/{key}   # where a `ticket` param links
investigations_dir: ~/notes/investigations           # <dir>/<run id>/OUTPUT.md write-ups
hidden_projects: [demo]                               # left off the jobs screens (H shows them)
```

```yaml
# ~/.config/agentd/agentd.yaml — the daemon
protected_orgs: [acme]   # narrow the push guard to these orgs (default: every remote)
```

Restart the daemon after changing `agentd.yaml`
(`beagle agent daemon restart`).

## Daemon flags

The launchd unit runs `beagle agent daemon run` with no flags. To run it
by hand in the foreground (stop the launchd one first), pass any of:

| Flag | Default | What it does |
|---|---|---|
| `--manifests DIR` | `~/.config/agentd/manifests` | The manifests root. |
| `--tick-seconds N` | `5` | Seconds between control-loop ticks. |
| `--once` | — | Run one tick and exit. |
| `--log-retention-hours N` | `4` | Keep a finished run's log and event feed this long (`0` = forever). The run row is kept. |
| `--notify all\|failures\|off` | `all` | Default notification policy; a manifest's `notify:` overrides it. |
| `--notify-icon PATH`, `--notify-icon-failure PATH` | stock icons | Default notification images. |
| `--notify-sound NAME`, `--notify-sound-failure NAME` | `Tink`, `Funk` | macOS system sounds. |

`beagle agent daemon run --help` prints the same list.

## Limits and retention

- **Machine-wide caps.** On top of each agent's `max_concurrent`, at most
  5 model runs (claude + codex) and 20 script runs are live at once,
  across every project. Over the cap a run stays `pending` and starts,
  oldest first, when a slot frees. No manifest can raise it
  (`daemon/src/caps.rs`).
- **Retention.** Logs are pruned after 4 hours. A run's state, result and
  `ERROR` lines stay in the store for a week, and so does its data
  directory — which is why `beagle agent resume` works for a week.
- **Heartbeat.** The daemon writes `agentd.heartbeat` every tick.
  `beagle agent status`, the dashboard and Agentdesk show a red banner
  when the daemon is stopped or stalled.
- **Keep-awake.** Runs keep the Mac from idle-sleeping (`caffeinate -i`)
  while they run. Closing the lid still sleeps it.

## Safety rails

Agents can do real work on your repos, so every run gets a few guard
rails. None of them can be switched off from inside a run.

**Git guard.** Every run gets agentd's `git` shim first on `PATH` and
`core.hooksPath` pointed at agentd's hook dispatcher (both written to
`~/.local/state/agentd/lib/` when the daemon starts, appended after the
run's own `GIT_CONFIG_*` pairs). The shim always refuses
`push --no-verify` and overriding `core.hooksPath`; the `pre-push` hook
refuses a push to `main`/`master` on every remote, or only on
`protected_orgs` repos when that is set (`$AGENTD_PROTECTED_ORGS` in the
daemon's environment overrides the file). The policy is written into the
hook at startup — a run cannot turn it off through its environment — and
the remote's URL is resolved with `git ls-remote --get-url`, so remote
names, `insteadOf` rewrites and SSH host aliases are all caught. Every
hook then runs the repo's own hook, so nothing is skipped. When a model
run finishes, its log notes the repos and branches it wrote to
(`repos touched: acme/widgets @ abc-123-fix (commit ×2, push)`), flagged
`⚠ PUSHED TO MAIN` if one got past the guard.

**GitHub write guard.** Agents read GitHub freely but cannot change it on
their own. Every run also gets agentd's `gh` shim first on `PATH`
(`~/.local/state/agentd/lib/ghguard/gh`), which refuses `gh pr merge`,
`gh pr review --approve` and any `gh api` call that is not a read (a
`-X`/`--method` other than GET, a request body via
`-f`/`-F`/`--field`/`--raw-field`/`--input`, or a GraphQL mutation) with
`agentd: BLOCKED gh …`. Claude runs also get the matching
`--disallowedTools` rules ahead of the manifest's own. A job that really
must write sets `allow_github_writes: true` in its manifest; there is no
environment switch.

**Secrets stay off the command line.** A run's environment (manifest
`env:`, params) reaches it through a 0600 file it sources and deletes,
never the command line `ps` shows; an `env_file:` is sourced by the run
itself.
