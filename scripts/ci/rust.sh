#!/usr/bin/env bash
# The Rust gate for one cargo workspace. CI and `make check` both run this
# script, so a green local run is a green CI run.
#
#   scripts/ci/rust.sh <workspace-dir> [--tests-only]   e.g. tandem, rca/desktop/src-tauri
#
# Test selection: with AFFECTED_BASE=<git-ref> set (CI sets it on pull
# requests to origin/<base branch>), `cargo test` runs only the packages the
# change affects — changed packages plus their reverse dependents — as
# computed by scripts/ci/affected.mjs. Unset (push to main, plain `make
# check`) tests the whole workspace. fmt, clippy and doc always cover the
# whole workspace. --tests-only skips them (used by `make test-affected`).
set -euo pipefail

dir=${1:?usage: scripts/ci/rust.sh <workspace-dir> [--tests-only]}
dir=${dir%/}
mode=${2:-}
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/$dir"

# Tauri's generate_context! refuses to compile when frontendDist is missing.
# This gate checks Rust only (the frontend gate builds the real bundle), so
# an empty placeholder is enough.
while IFS= read -r conf; do
  dist=$(sed -n 's/.*"frontendDist"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$conf")
  [ -n "$dist" ] && mkdir -p "$(dirname "$conf")/$dist"
done < <(find . -maxdepth 2 -name tauri.conf.json -not -path './target/*')

# Sets test_args to the `cargo test` package selection; empty = skip tests.
select_tests() {
  test_args=(--workspace)
  [ -n "${AFFECTED_BASE:-}" ] || return 0
  command -v node >/dev/null 2>&1 || {
    echo "AFFECTED_BASE is set but node is not on PATH (needed by scripts/ci/affected.mjs)" >&2
    exit 1
  }
  local selection
  selection=$(node "$root/scripts/ci/affected.mjs" "$dir" --base "$AFFECTED_BASE")
  if [ "$selection" = all ]; then
    return 0
  fi
  test_args=()
  local pkg
  for pkg in $selection; do test_args+=(-p "$pkg"); done
}

if [ "$mode" != --tests-only ]; then
  echo "==> [$dir] rustfmt"
  cargo fmt --all -- --check
  echo "==> [$dir] clippy (zero warnings)"
  cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
fi

select_tests
if [ "${#test_args[@]}" -eq 0 ]; then
  echo "==> [$dir] tests: skipped (no affected package)"
else
  echo "==> [$dir] tests (${test_args[*]})"
  cargo test "${test_args[@]}" --all-features --locked
fi

if [ "$mode" != --tests-only ]; then
  echo "==> [$dir] docs (zero warnings)"
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
fi
echo "OK: [$dir] rust gate green"
