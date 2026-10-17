# Agent sandboxing — v1 reality, v2 plan

## v1 (current): local processes, trusted like your own shell

Agents run as local child processes (`claude -p`, `codex exec`, custom
commands). That means they run with your user's permissions and your
network. This matches how `agentd` (this monorepo's `workflows/`) runs
today and is fine
for a single-user local tool, but the isolation is **trust-based, not
enforced**:

- The run context tells the agent it must not post to GitHub — nothing
  _prevents_ a malicious/buggy agent from doing so.
- `AgentSpec.network_allowlist` is collected in the UI **but not
  enforced** in v1. It exists so specs are already sandbox-shaped. New
  agents are pre-seeded with their runner's provider endpoints (claude →
  `api.anthropic.com` + `claude.ai`; codex → `api.openai.com` +
  `chatgpt.com` + `auth.openai.com`) so most users never edit it.
- Mitigations available today: claude's `--allowedTools` allowlist,
  per-run working directories, and hard timeouts.

## v2 (required): Docker + squid egress proxy

> **This is a committed v2 deliverable, not a nice-to-have.** The whole
> point is that the agent can do whatever it wants _inside_ its
> environment — because the environment can't reach anything you didn't
> allow.

Design sketch:

1. **Container per run.** The runner gains a `DockerSandboxRunner`
   implementing the same interface as `LocalProcessRunner` (spawn →
   stream events → comments file → timeout/cancel). The run directory is
   bind-mounted as the only writable volume; the diff and prompt arrive
   the same way they do today (stdin + files in the run dir).
2. **Egress via squid only.** The container has no direct network. Its
   only route is a squid sidecar; squid's ACL is generated from
   `AgentSpec.network_allowlist` (plus the provider endpoint implied by
   the spec's `AuthMode`). Deny by default, log every request — the
   squid access log becomes part of the run's log bundle in the UI.
3. **Credentials.** API keys are injected as env vars into the container
   only (never written into the image or run dir). CLI-session auth
   (claude/codex login state) is mounted read-only, and only when the
   spec's auth mode asks for it.
4. **Runner images.** Small base images with the claude/codex CLIs
   preinstalled; custom-command specs can name their own image.
5. **UX.** The agent editor's network-allowlist field switches from
   "informational" to "enforced"; runs display a per-run egress log tab.

Prior art to lean on: the dotfiles-ai repo's `codex/infra` (command guardrails +
"the real boundary must be the environment, not command rules") and its
colima-based Docker setup on macOS.

## Invariants that hold in both versions

- Agent output is local-only; GitHub writes happen solely via explicit
  user actions (post / approve) — never from a run.
- Every run: own directory, JSONL event log, timeout, cancel handle.
- Secrets never land in the cache DB or event logs.
