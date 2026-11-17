#!/usr/bin/env bash
# Keeps employer- and client-specific names out of this public repo:
# company names, private org/repo names, internal hostnames, ticket keys,
# account ids, work email domains.
#
# The patterns themselves are never committed (listing them here would be
# the leak). They are extended regexes, one per line, `#` for comments,
# matched case-insensitively, read from the first of:
#   1. $BEAGLE_AFFILIATION_PATTERNS   newline-separated (CI: a repo secret)
#   2. $BEAGLE_AFFILIATION_FILE       a path
#   3. ~/.config/beagle-tools/affiliation-patterns
#
# Checks every tracked and untracked-but-not-ignored file, plus the author
# and committer emails of commits not yet on origin/main. With no patterns
# it warns and passes, unless BEAGLE_AFFILIATION_REQUIRED=1 (set in CI).
set -euo pipefail
cd "$(dirname "$0")/.."

patterns="${BEAGLE_AFFILIATION_PATTERNS:-}"
if [ -z "$patterns" ]; then
  file="${BEAGLE_AFFILIATION_FILE:-$HOME/.config/beagle-tools/affiliation-patterns}"
  [ -f "$file" ] && patterns="$(cat "$file")"
fi
patterns="$(printf '%s\n' "$patterns" | sed -e 's/#.*//' -e 's/[[:space:]]*$//' | grep -v '^$' || true)"

if [ -z "$patterns" ]; then
  if [ "${BEAGLE_AFFILIATION_REQUIRED:-0}" = "1" ]; then
    echo "FAIL: no affiliation patterns configured (BEAGLE_AFFILIATION_PATTERNS)"
    exit 1
  fi
  echo "SKIP: no affiliation patterns (see scripts/check-affiliation.sh)"
  exit 0
fi

pattern_file="$(mktemp)"
trap 'rm -f "$pattern_file"' EXIT
printf '%s\n' "$patterns" >"$pattern_file"

failed=0
# -l only: printing the matching line would echo the pattern into CI logs.
# Judged by output, not status: xargs exits non-zero when any batch of
# files has no match, which says nothing about the other batches.
hits="$(git ls-files -z --cached --others --exclude-standard |
  xargs -0 grep -IliE -f "$pattern_file" -- 2>/dev/null || true)"
if [ -n "$hits" ]; then
  echo "FAIL: affiliation pattern found in:"
  printf '  %s\n' $hits
  failed=1
fi

base="$(git merge-base HEAD origin/main 2>/dev/null || true)"
range="${base:+$base..}HEAD"
if git log --format='%ae%n%ce' "$range" | grep -qiE -f "$pattern_file"; then
  echo "FAIL: a commit in $range has an affiliated author/committer email"
  failed=1
fi
if git config user.email | grep -qiE -f "$pattern_file"; then
  echo "FAIL: git user.email for this repo is affiliated; set a personal one"
  failed=1
fi

[ "$failed" -eq 0 ] && echo "OK: no affiliation patterns found"
exit "$failed"
