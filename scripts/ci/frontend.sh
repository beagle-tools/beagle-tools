#!/usr/bin/env bash
# The frontend gate for one pnpm package. CI and `make check` both run this
# script, so a green local run is a green CI run.
#
#   scripts/ci/frontend.sh <package-dir>    e.g. tandem, rca/web
#
# The e2e step needs Playwright's Chromium: CI installs it in _frontend.yml;
# locally `make bootstrap` does (or `pnpm exec playwright install chromium`).
set -euo pipefail

dir=${1:?usage: scripts/ci/frontend.sh <package-dir>}
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/$dir"

# pnpm comes from corepack (pinned by package.json's packageManager) when it
# isn't already on PATH.
if command -v pnpm >/dev/null 2>&1; then pnpm=(pnpm); else pnpm=(corepack pnpm); fi

echo "==> [$dir] install (frozen lockfile)"
"${pnpm[@]}" install --frozen-lockfile
echo "==> [$dir] prettier"
"${pnpm[@]}" format:check
echo "==> [$dir] eslint (zero warnings)"
"${pnpm[@]}" lint
echo "==> [$dir] typecheck"
"${pnpm[@]}" typecheck
echo "==> [$dir] unit tests (vitest)"
"${pnpm[@]}" test
echo "==> [$dir] production build"
"${pnpm[@]}" build
echo "==> [$dir] e2e (playwright, chromium)"
"${pnpm[@]}" e2e
echo "OK: [$dir] frontend gate green"
