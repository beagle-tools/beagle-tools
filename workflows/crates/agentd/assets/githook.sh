#!/bin/sh
# agentd's git hook dispatcher. Every agentd run (script or model) runs git
# with core.hooksPath pointing at a directory of links to this one file, so
# it fires for every hook name. It does two things:
#
#   1. pre-push: refuses a push to main or master on a protected org's repo
#      ($AGENTD_PROTECTED_ORGS, space-separated; agentd sets it from
#      `protected_orgs:` in ~/.config/agentd/agentd.yaml — empty protects
#      nothing). Headless runs must go through a branch and a PR.
#   2. every hook: runs the repo's OWN hook of the same name afterwards —
#      its local core.hooksPath (husky) or .git/hooks — so overriding
#      hooksPath never skips a repo's secret scan or lint.
set -u
name=$(basename "$0")
PROTECTED_ORGS="${AGENTD_PROTECTED_ORGS:-}"

input=""
if [ "$name" = "pre-push" ]; then
  input=$(cat)
  url=$(printf '%s' "${2:-}" | tr '[:upper:]' '[:lower:]')
  for org in $(printf '%s' "$PROTECTED_ORGS" | tr ',' ' ' | tr '[:upper:]' '[:lower:]'); do
    case "$url" in
      *github.com[:/]"$org"/*)
        printf '%s\n' "$input" | while read -r _lref _lsha rref _rsha; do
          case "$rref" in
            refs/heads/main|refs/heads/master)
              echo "agentd: BLOCKED push to ${rref#refs/heads/} on $2 — $org repos never take a direct push from an agent; push a branch and open a PR" >&2
              exit 1 ;;
          esac
        done || exit 1 ;;
    esac
  done
fi

# The repo's own hook: a repo-local hooksPath (relative paths are from the
# worktree top, where git runs hooks), else the common .git/hooks.
own=$(git config --local --get core.hooksPath 2>/dev/null || true)
[ -n "$own" ] || own="$(git rev-parse --git-common-dir 2>/dev/null)/hooks"
hook="$own/$name"
[ -x "$hook" ] || exit 0
if [ "$name" = "pre-push" ]; then
  printf '%s\n' "$input" | "$hook" "$@"
else
  exec "$hook" "$@"
fi
