#!/usr/bin/env bash
# Creates or updates the labels the SDLC relies on (docs/SDLC.md).
# Idempotent: `gh label create --force` updates color/description in place.
#
#   scripts/github/labels.sh                 # current repo (from the cwd)
#   scripts/github/labels.sh owner/repo      # explicit repo
set -euo pipefail

repo_args=()
if [ $# -gt 0 ]; then
  repo_args=(--repo "$1")
fi

command -v gh >/dev/null || { echo "error: gh CLI not found" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || {
  echo "error: gh is not logged in — run 'gh auth login'" >&2
  exit 1
}

# name|color|description
labels=(
  "app:tandem|1d76db|Tandem (tandem/) — PR review desktop app"
  "app:rca|0e8a16|Beagle RCA (rca/) — CLI/TUI, agent daemon, desktop, web"
  "app:workflows|5319e7|agentd + Agentdesk (workflows/)"
  "app:utils|c5def5|Shared beagle-utils crate (utils/) — releases every app"
  "app:cli|bfd4f2|The beagle base CLI (cli-base/) — released as beagle-cli-v*"
  "release:major|b60205|Merging cuts a major release of the app (breaking change)"
  "release:minor|fbca04|Merging cuts a minor release of the app (new feature)"
  "release:patch|c2e0c6|Merging cuts a patch release of the app (fix/internal)"
  "agent-ready|0052cc|Specified precisely enough for /beagle-complete-issue"
  "in-progress|f9d0c4|Claimed — an agent or human is working on it"
  "blocked|d93f0b|Waiting on another issue or a human decision"
)

for entry in "${labels[@]}"; do
  IFS='|' read -r name color description <<<"$entry"
  gh label create "$name" --color "$color" --description "$description" \
    --force "${repo_args[@]+"${repo_args[@]}"}" >/dev/null
  echo "ok: $name"
done
