#!/usr/bin/env bash
# The Rust gate for one cargo workspace. CI and `make check` both run this
# script, so a green local run is a green CI run.
#
#   scripts/ci/rust.sh <workspace-dir>     e.g. tandem, rca/desktop/src-tauri
set -euo pipefail

dir=${1:?usage: scripts/ci/rust.sh <workspace-dir>}
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root/$dir"

# Tauri's generate_context! refuses to compile when frontendDist is missing.
# This gate checks Rust only (the frontend gate builds the real bundle), so
# an empty placeholder is enough.
while IFS= read -r conf; do
  dist=$(sed -n 's/.*"frontendDist"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$conf")
  [ -n "$dist" ] && mkdir -p "$(dirname "$conf")/$dist"
done < <(find . -maxdepth 2 -name tauri.conf.json -not -path './target/*')

echo "==> [$dir] rustfmt"
cargo fmt --all -- --check
echo "==> [$dir] clippy (zero warnings)"
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
echo "==> [$dir] tests"
cargo test --workspace --all-features --locked
echo "==> [$dir] docs (zero warnings)"
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
echo "OK: [$dir] rust gate green"
