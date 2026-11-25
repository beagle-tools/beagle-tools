# Writing manifests

The full reference for agentd manifests: every field, how jobs and steps
differ, declared parameters, and what a run sees in its environment. For a
first tour, start with the [README](../README.md).

The source of truth is `lib/core/src/manifest.rs` (the YAML shape) and
`lib/core/src/types.rs` (what each field means). Manifests are parsed with
`deny_unknown_fields`: a typo, or a field newer than the running daemon,
gets the file rejected.

## Where manifests live

```
~/.config/agentd/manifests/        the root ($AGENTD_MANIFESTS_DIR moves it)
  demo/                            a project — the directory name is its name
    project.yaml                   optional: kind: Project, description: …
    fanout.yaml                    one agent per *.yaml / *.yml file
    report-claude.yaml
    prompts/report.md              anything else the agents point at
    scripts/fanout.sh
    jobs/fanout/job-purpose.md     every job's purpose file (see below)
```

Paths inside a manifest resolve relative to the manifest file, and `~`
expands. One bad file is reported and skipped; the rest still load.
`beagle agent apply` prints every problem as a `WARN` line and exits
non-zero when there were any.

`scripts/agentman` (bash, not shipped in `beagle`) is an optional helper
for people who keep projects in several repos: it registers manifest
roots and links their projects into the one root agentd reads.

## Agent fields

Only `kind`, `name`, `runner` and `workdir` are required.

| Field | Type / values | Default | What it does |
|---|---|---|---|
| `kind` | `Agent` | — | Required. (`project.yaml` uses `kind: Project`.) |
| `name` | string | — | Unique within the project; the run is `<project>/<name>`. |
| `runner` | `claude` \| `codex` \| `script` | — | `claude -p` headless, `codex exec`, or a plain shell `command`. |
| `workdir` | path | — | Where the run executes. |
| `prompt` | path | — | Model runners: a prompt file, or a directory whose `.md` files are joined in name order. |
| `command` | string | — | `script` runner: the shell command, run in `workdir`. |
| `model` | string | the CLI's default | Passed as `--model` / `-m`. `-m` / `--model` on the command line overrides it per run. |
| `schedule` | 5-field cron | manual only | When the daemon starts it on its own. |
| `timezone` | `utc` \| `local` | `utc` | The clock `schedule` is read in. `local` follows this Mac's clock, daylight saving included. |
| `timeout_minutes` | integer | `30` | Wall-clock limit; the daemon then kills the run as `timedout`. |
| `max_concurrent` | integer | unlimited | At most this many running runs of this agent; extras wait as `pending`. |
| `concurrency` | `forbid` \| `allow` | `forbid` | What a cron tick does while a previous run is still active: skip it, or start another. |
| `enabled` | bool | `true` | `false` keeps the agent loaded but never schedules it. |
| `isolation` | `none` \| `worktree` | `none` | `worktree` runs in a fresh detached `git worktree` of `workdir`, cleaned up afterwards. |
| `env` | map | — | Extra environment variables for the run. |
| `env_file` | path | — | Dotenv-style file sourced before `env` — keeps secrets out of the manifest. |
| `database` | bool | `false` | A persistent per-agent SQLite file, exported as `$AGENTD_DB` (handy for pollers and dedup). |
| `params` | list | free-form | Declared job parameters; see below. |
| `entrypoint` | bool | inferred | Force "this is a job" (`true`) or "this is a step" (`false`); see below. |
| `triggers` | list of agent names | — | The agents (same project) this one may start. Drawn by the views; not enforced. |
| `noop_retention_minutes` | integer | — | Delete runs that succeeded without announcing anything this long after they finish (quiet poller ticks). The newest run is kept. |
| `notify` | `all` \| `failures` \| `off` | daemon's `--notify` (`all`) | macOS notifications for finished runs. |
| `notify_icon`, `notify_icon_failure` | path | stock icons | Notification images (need `terminal-notifier`). |
| `allow_github_writes` | bool | `false` | Lift the GitHub write guard for this agent ([configuration.md](configuration.md#safety-rails)). |

Claude-only fields:

| Field | What it does |
|---|---|
| `allowed_tools` | `--allowedTools`: what the headless run may use without asking. Without it, a headless run can do very little. |
| `disallowed_tools` | `--disallowedTools`: deny wins, so this is how you say "all of git except push". |
| `append_system_prompt` | A file passed as `--append-system-prompt`. |
| `add_dirs` | Extra directories (`--add-dir`). |
| `mcp_config` | Pin the run to the MCP servers in this JSON file (`--mcp-config` + `--strict-mcp-config`); every other configured server is ignored. |
| `requires_mcp` | MCP server names (as `claude mcp get <name>` knows them) that must be connected before `-j` queues a run. A server that needs sign-in is refused up front with the fix. |

Codex-only: `sandbox: read-only | workspace-write` (default off) passes
`--sandbox`. `workspace-write` limits writes to the workdir and the run's
data dir and blocks the network.

## Jobs, steps and `job-purpose.md`

A **job** is something a person or the clock starts. A **step** is an
agent another agent triggers (`$AGENTD_CTL run <project>/<agent> --parent
$AGENTD_RUN_ID`). An agent counts as a job when it is scheduled, declares
`entrypoint: true`, or no sibling lists it in `triggers` (unless it says
`entrypoint: false`).

Every job needs `<project>/jobs/<job>/job-purpose.md`, or it **does not
load** (`apply` reports it). Steps need none — their job's file covers
them. Sections:

```markdown
# <project>/<job>
## How it starts
## Steps
## Where things live
## Known failure modes
## Rules a repair must keep
## Verify a fix
```

agentd puts the opening sections (what the job is for, how it starts, its
steps) at the top of every model step's prompt, with the path to the whole
file; scripts get the path as `$AGENTD_JOB_PURPOSE`. Real examples:
[`../examples/demo/jobs/`](../examples/demo/jobs/).

## Parameters

```yaml
params:
  - name: ticket
    required: true
    description: Tracker key, e.g. ABC-123
  - name: depth
    default: "2"
```

With no `params:` block, any `-p` is passed through unchecked. Once a job
declares params the set is closed: `beagle agent -j <job>` refuses a run
that is missing a required one or names one it does not declare, and says
what to pass. `beagle agent get jobs` lists each job's params.

```sh
beagle agent -j triage -p ticket=ABC-123,depth=3   # comma-separate, or repeat -p
beagle agent -j triage -p ticket=ABC-123 -m opus   # override the model once
```

Each param reaches the run as `AGENTD_PARAM_<NAME>` (upper-cased,
non-alphanumerics become `_`). `beagle agent run <project>/<agent>
--param K=V` skips the declared-params check; it is what scripts use to
start steps.

## What a run sees

| Variable | Value |
|---|---|
| `AGENTD_RUN_ID`, `AGENTD_PROJECT`, `AGENTD_AGENT` | Who this run is. |
| `AGENTD_ROOT_RUN_ID` | The first run of the chain. |
| `AGENTD_DATA_DIR` | This run's data directory — the handover to the steps it starts. |
| `AGENTD_PARENT_RUN_ID`, `AGENTD_PARENT_DATA_DIR` | Set when started with `--parent`: read the parent's handover here. |
| `AGENTD_PARAM_<NAME>` | One per param. |
| `AGENTD_DB` | With `database: true`. |
| `AGENTD_CTL` | The control command from the daemon's own install (`<beagle> agent`). Start steps with `$AGENTD_CTL run …`, never a hard-coded `agentctl`. |
| `AGENTD_JOB_PURPOSE` | The job's `job-purpose.md`. |
| `AGENTD_LOG_SH` | A small shell logger: `. "$AGENTD_LOG_SH"`, then `log_info`, `log_step`, `log_ok`, `log_warn`, `log_error`. |

Plus the manifest's `env_file` and `env`, and `~/.cargo/bin` and
`~/.local/bin` ahead of the daemon's `PATH`. The variables above, `env:`
and params are handed over through a private file the run sources and
deletes, so they never appear on a command line `ps` can show.

## Building a DAG

Branching lives in plain code. A `script` agent decides what happens
next and starts the next steps itself:

```sh
# examples/demo/scripts/fanout.sh (abridged)
df -h / | tail -1 > "$AGENTD_DATA_DIR/finding.txt"
${AGENTD_CTL:-agentctl} run demo/report-claude --parent "$AGENTD_RUN_ID"
${AGENTD_CTL:-agentctl} run demo/report-codex  --parent "$AGENTD_RUN_ID"
```

The children read `$AGENTD_PARENT_DATA_DIR/finding.txt`. `--parent`
records the lineage, which is what the dashboard and Agentdesk draw, and
what `resume` uses to re-run a failed step in place. Declare
`triggers: [report-claude, report-codex]` on the parent (or
`entrypoint: false` on the children) so the views know the shape before
anything has run.
