# Roadmap

## v1 (now) — the review loop

- [x] Tauri 2 + Rust workspace + React scaffold, strict CI
- [x] GitHub read-only client (gh CLI / PAT / GHES base URL)
- [x] SQLite cache-first data flow with visible sync state
- [x] Structured differ with inline local comments
- [x] Agent runs: headless claude / codex / custom command, live JSONL
      event feed, tandem_comment contract, triage workflow
- [ ] Comment anchors survive force-pushes (re-anchor by hunk context)
- [ ] Syntax highlighting in the differ
- [ ] Split (side-by-side) diff view
- [ ] OAuth device-flow auth (alongside gh CLI and PAT)
- [ ] PAT/API keys in OS keychain instead of settings file

## v2 — enforced sandboxing (committed)

- [ ] **Docker + squid egress sandbox for agent runs** — see
      [SANDBOXING.md](SANDBOXING.md). `network_allowlist` becomes
      enforced; per-run egress logs in the UI.
- [ ] Runner images for claude/codex; custom images for custom agents
- [ ] Read-only mounts for CLI login state

## Later

- GitLab / other forges behind the same domain types
- Multi-agent panels (compare two agents' reviews side by side)
- Export accepted comments as a markdown review (manual paste-to-GitHub)
- Shareable agent spec manifests (agentd-compatible YAML)
