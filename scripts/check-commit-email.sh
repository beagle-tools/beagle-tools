#!/usr/bin/env bash
# This repo is public: no personal email address may appear in it. Every
# commit not yet on origin/main must be authored and committed with a GitHub
# noreply address (`<id>+<login>@users.noreply.github.com`, or GitHub's own
# `noreply@github.com` for the merge commits CI checks out), and this
# checkout's `user.email` must be one too.
#
# Fix a checkout with:
#   git config user.email "<id>+<login>@users.noreply.github.com"
# (GitHub shows yours under Settings → Emails.)
set -euo pipefail
cd "$(dirname "$0")/.."

noreply='(@users\.noreply\.github\.com|^noreply@github\.com)$'
failed=0

base="$(git merge-base HEAD origin/main 2>/dev/null || true)"
range="${base:+$base..}HEAD"
if [ -n "$base" ]; then
  # Only the count, never the address: CI logs are public.
  bad="$(git log --format='%ae%n%ce' "$range" | grep -cviE "$noreply" || true)"
  if [ "$bad" -gt 0 ]; then
    echo "FAIL: $bad author/committer email(s) in $range are not GitHub noreply addresses"
    echo "      set user.email (see the top of this script) and amend or rebase those commits"
    failed=1
  fi
fi

email="$(git config user.email || true)"
if [ -n "$email" ] && ! printf '%s\n' "$email" | grep -qiE "$noreply"; then
  echo "FAIL: this checkout's git user.email is not a GitHub noreply address"
  echo "      run: git config user.email \"<id>+<login>@users.noreply.github.com\""
  failed=1
fi

[ "$failed" -eq 0 ] && echo "OK: commits use GitHub noreply addresses"
exit "$failed"
