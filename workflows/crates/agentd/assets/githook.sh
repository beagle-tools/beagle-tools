#!/bin/sh
# agentd's git hook dispatcher. Every agentd run (script or model) runs git
# with core.hooksPath pointing at a directory of links to this one file, so
# it fires for every hook name. It does two things:
#
#   1. pre-push: refuses a push to main or master. The policy is written
#      into this file when agentd starts (from `protected_orgs:` in
#      ~/.config/agentd/agentd.yaml), never read from the environment, so a
#      run cannot switch it off: `*` (the default, no orgs configured)
#      guards every remote; a list guards only those orgs' repos, on any
#      host (an SSH alias like `work:acme/widgets` still names the org).
#      Both the URL git passes and `git ls-remote --get-url <remote>` are
#      checked. Headless runs must go through a branch and a PR.
#   2. every hook: runs the repo's OWN hook of the same name afterwards —
#      its local core.hooksPath (husky) or .git/hooks — so overriding
#      hooksPath never skips a repo's secret scan or lint.
set -u
name=$(basename "$0")
PROTECTED_ORGS='__AGENTD_PROTECTED_ORGS__'

# Whether a push to remote $1 (a name or URL) at URL $2 is guarded.
guarded() {
  [ "$PROTECTED_ORGS" = "*" ] && return 0
  urls=$(printf '%s\n%s\n' "${2:-}" "$(git ls-remote --get-url "${1:-}" 2>/dev/null)" |
    tr '[:upper:]' '[:lower:]')
  for org in $(printf '%s' "$PROTECTED_ORGS" | tr ',' ' ' | tr '[:upper:]' '[:lower:]'); do
    for url in $urls; do
      case "$url" in
        *[:/]"$org"/*) return 0 ;;
      esac
    done
  done
  return 1
}

input=""
if [ "$name" = "pre-push" ]; then
  input=$(cat)
  if guarded "${1:-}" "${2:-}"; then
    printf '%s\n' "$input" | while read -r _lref _lsha rref _rsha; do
      case "$rref" in
        refs/heads/main|refs/heads/master)
          echo "agentd: BLOCKED push to ${rref#refs/heads/} on ${2:-${1:-}} — agent runs never push to main directly; push a branch and open a PR" >&2
          exit 1 ;;
      esac
    done || exit 1
  fi
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
