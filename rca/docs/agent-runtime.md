# The Beagle RCA agent daemon

An always-on, native-Rust daemon that turns **RCAs you mark ready into
remediation PRs**, plus the TUI and desktop screens to watch and steer it.
You find the root cause; the daemon fetches the fix.

```
RCA at status `agent`  ──▶  beagle-agentd  ──▶  isolated git worktree
                                                     │
                                       headless `claude -p` implements
                                       remediation.md and commits
                                                     │
                    Rust pushes agent/<id> → opens a PR → attaches it
                                       → moves the RCA to final-review
```

Deterministic Rust owns every risky step (git, GitHub, RCA status); `claude`
only edits code. A run is isolated in its own worktree and OS process group, is
supervised (a panic or failure never takes the daemon down), retries under a
cap, and cleans up after itself — a crash never leaks a worktree or a
token-burning session.

## Layout

```text
lib/agent/              the engine: config, sqlite job store, RCA poller,
                        git-worktree manager, headless claude runner, the
                        orchestrator, and the control protocol + client/server
daemon/                 the always-on daemon that hosts the engine and serves
                        the control socket
tui/  (beagle)          `beagle rca agent …` — install/uninstall/start/stop/status,
                        and the TUI's Agents screen
desktop/                the desktop app's Agents view (Tauri commands + a live
                        event stream)
```

## Before you start

- `claude` (Claude Code) on your `PATH`, logged in: `claude --version` works.
- `gh` logged in: `gh auth status` works. The daemon uses it to open PRs.
- A local clone of the repo the fixes go into, with an `origin` remote that
  has a `main` branch. Each run branches off `origin/main`.

The daemon never logs in for you. If either check fails, it skips the tick
and says why in `beagle rca agent status`.

## Quick start

```sh
# 1. Install `beagle`: the daemon is `beagle rca agentd run` inside it.
brew install beagle-tools/tap/beagle

# 2. Write a prompt: your house rules for implementing a fix.
mkdir -p ~/.config/beagle/prompts
$EDITOR ~/.config/beagle/prompts/rca-remediation.txt

# 3. Configure at least one agent (the format is below).
$EDITOR ~/.config/beagle/agents.toml

# 4. Try one tick in the foreground, then autostart it
#    (launchd on macOS, systemd --user on Linux).
beagle rca agentd once
beagle rca agent install

# 5. Watch it: `m` in the TUI, **Agents** in the desktop app, or:
beagle rca agent status
```

Then move a reviewed RCA to `agent` (`beagle rca status <id> agent`, or `t` in
the TUI) and the daemon picks it up on its next poll.

A starting prompt might be:

```text
You are fixing a production issue described by the RCA below, in this
repository. Implement the durable fix from remediation.md. Keep the change
small and focused. Run the project's tests (see its README or Makefile) and
fix anything you broke. Commit your work with a clear message. Do not push.
```

## Configuring agents (`agents.toml`)

The config lives at `~/.config/beagle/agents.toml` (or
`$XDG_CONFIG_HOME/beagle/agents.toml`). It is optional — an absent file means
"no agents", and the daemon idles. Unknown keys are rejected, so typos surface
immediately. One `[[agent]]` table per agent:

```toml
[[agent]]
# Stable id, unique in the file. Used as the job key and shown in the UI.
id = "rca-remediation"

# Whether the daemon runs this agent. Defaults to true.
enabled = true

# What makes an RCA actionable. v1: an RCA sitting at this lifecycle status.
trigger = { kind = "rca-status", status = "agent" }

# The git repository the agent implements changes in. `~/` is expanded.
target_repo = "~/src/widgets"

# Required: the prompt file handed to the headless claude session (see below).
prompt = "~/.config/beagle/prompts/rca-remediation.txt"

# The --allowedTools list for the session. Empty inherits your Claude Code
# settings. The daemon pushes and opens the PR itself, so the agent needs
# git commit but never `gh` or `git push`: those are always disallowed, and
# the session gets no GitHub credentials.
allowed_tools = ["Read", "Edit", "Bash(git status:*)", "Bash(git diff:*)", "Bash(git add:*)", "Bash(git commit:*)", "Bash(cargo:*)"]

# How often the trigger is polled. Accepts ms / s / m / h. Defaults to 60s.
poll_interval = "60s"

# The most concurrent sessions this agent may run. Defaults to 2.
max_concurrent = 2
```

Edit it any time. From the UIs, editing and reloading is one step (the TUI's
`e`, the desktop's **Reload config**); by hand, run `beagle rca agent status` after
a restart, or send a reload from a UI. Enable/disable and pause/resume changes
apply live; adding or removing an `[[agent]]` needs a daemon restart.

### Custom prompts

`prompt` points at a plain-text file — the base instructions for the session.
The daemon appends the RCA's id, title, and `remediation.md` beneath it, then
runs `claude -p` in the isolated worktree. Keep the prompt focused on *how* to
implement a fix in `target_repo` (house rules, verification commands); the
per-RCA specifics come from `remediation.md`. The session must commit: a run
that leaves no new commit opens no PR and counts as a failure.

## Autostart & control: `beagle rca agent`

```sh
beagle rca agent install      # write + load the service (starts on login, restarts on crash)
beagle rca agent uninstall    # unload + remove it
beagle rca agent start        # (re)start the loaded service
beagle rca agent stop         # stop it (stays stopped until start/install)
beagle rca agent status       # query the running daemon over its control socket
```

- **macOS** installs a launchd `LaunchAgent` (`~/Library/LaunchAgents/com.beagle.agentd.plist`,
  `RunAtLoad` + `KeepAlive`).
- **Linux** installs a `systemd --user` unit
  (`~/.config/systemd/user/beagle-agentd.service`, `Restart=always`,
  `WantedBy=default.target`). To keep it running without an active login:
  `loginctl enable-linger $USER`.

The unit runs `<beagle> rca agentd run` (the binary's upgrade-proof path,
never a Homebrew keg); `$BEAGLE_AGENTD` overrides the program. Its working
directory is the RCA store `beagle rca` resolves when you run `install`, so
run `install` from where `beagle rca` shows your RCAs, and run it again after
moving the store.

The daemon also runs directly: `beagle rca agentd run` loops;
`beagle rca agentd once` runs a single tick per agent and exits (handy for a
manual end-to-end). The deprecated standalone `beagle-agentd` takes the same
`run`/`once`; re-run `beagle rca agent install` to move a service off it.

## Watching agents

Both UIs have a dedicated **Agents screen**, separate from the RCA browser, that
never disturbs it:

- **TUI** — press `m` (`m`/`Esc` to go back). `j`/`k` select an agent, `x`
  starts/stops it, `Enter` opens its latest session log, `e` edits `agents.toml`
  and reloads. It shows daemon health, each agent's running/paused state, active
  sessions, last tick, and recent outcomes, auto-refreshing every ~2s.
- **Desktop** — click **Agents** in the sidebar (**← RCAs** to go back).
  Per-agent **Start/Stop**, a **Reload config** button, and the same live
  status, streamed over Tauri events.

A down daemon shows a clean offline banner in both — it never hangs the UI.

## How it works (contracts)

- **Control socket** — a loopback unix socket
  (`$XDG_RUNTIME_DIR/beagle-agentd.sock`, else
  `~/.local/state/beagle/agentd.sock`). Never a network port. It speaks
  newline-delimited JSON: `get-status`, `list-agents`, `start-agent`,
  `stop-agent`, `reload-config`, and `subscribe` (a live push stream). The TUI
  and desktop are thin clients over it.
- **State** — under `$XDG_STATE_HOME/beagle-agent` (else
  `~/.local/state/beagle-agent`): `jobs-<id>.db` (SQLite job state,
  `pending → processing → done/failed`, reset on boot), `logs/<agent>/<rca>.log`
  (per-session output), `runs/` (live-session registry for orphan cleanup),
  `worktrees/<agent>/<rca>` (isolated checkouts).
- **Auth** — the daemon is passive: it never triggers a login. It idle-skips a
  tick unless `gh auth status` and `claude --version` both succeed.
- **A run** — the worktree is a new branch `agent/<rca-id>` off `origin/main`.
  If the session succeeds and committed, the daemon pushes the branch, opens a
  PR titled `agent: remediate <id> — <title>`, attaches it to the RCA and sets
  `final-review`. A failed run is retried up to 3 attempts in all, then marked
  `failed`. The worktree is always removed.
- **RCA store root** — resolved like `beagle rca` does: `$BEAGLE_AGENT_RCA_ROOT`,
  else `$BEAGLE_RCA_ROOT`, else `root` from the effective config (nearest
  `.beagle`, else the global config), else the nearest directory with `rcas/`. `beagle rca agent install`
  sets the service's working directory to the store it resolves, so the
  daemon finds the same RCAs as the CLI (`agentd::store_root`).

## Troubleshooting

| Symptom | Check |
|---|---|
| `agent status` says the daemon is offline | `beagle rca agent start`, or run `beagle rca agentd run` in a terminal to see errors |
| Status shows `idle-skip: …` | `claude --version` or `gh auth status` failed in the daemon's environment. Log in, and check `PATH` in the service file holds both tools (re-run `beagle rca agent install` to refresh it) |
| An RCA at `agent` is never picked up | The daemon may be watching another store: run `beagle rca agent install` again from your store, or set `BEAGLE_AGENT_RCA_ROOT`. Also check the agent is `enabled` and its trigger status is `agent` |
| A run fails | Read the session log in `logs/<agent>/<rca>.log` under the state directory. Common causes: the prompt file is missing, `target_repo` has no `origin/main`, or the session made no commit |
| Daemon output | macOS: `~/.local/state/beagle-agent/logs/agentd.out.log` and `agentd.err.log`. Linux: `journalctl --user -u beagle-agentd` |
| `agents.toml` edits do nothing | Enable/disable and reload apply live; adding or removing an `[[agent]]` needs `beagle rca agent stop` then `start` |
