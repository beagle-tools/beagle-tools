#!/usr/bin/env bash
# Packs one target's CLI binaries into `<prefix>-<target>.tar.gz` plus a
# `.sha256` beside it. The tarball root holds the binaries directly (no
# wrapping directory) — the Homebrew formulas `bin.install` them by name.
#
#   scripts/release/package-cli.sh <prefix> <target> <bin-dir> <out-dir> <bin>...
#
# e.g. package-cli.sh beagle-rca aarch64-apple-darwin \
#        rca/target/aarch64-apple-darwin/release dist beagle beagle-agentd
set -euo pipefail

prefix=${1:?usage: package-cli.sh <prefix> <target> <bin-dir> <out-dir> <bin>...}
target=${2:?missing target}
bin_dir=${3:?missing bin dir}
out_dir=${4:?missing out dir}
shift 4
[ "$#" -gt 0 ] || { echo "no binaries named" >&2; exit 1; }

mkdir -p "$out_dir"
out_dir="$(cd "$out_dir" && pwd)"
stage=$(mktemp -d)
for bin in "$@"; do
  [ -x "$bin_dir/$bin" ] || { echo "missing binary $bin_dir/$bin" >&2; exit 1; }
  cp "$bin_dir/$bin" "$stage/"
done

name="$prefix-$target.tar.gz"
tar -C "$stage" -czf "$out_dir/$name" "$@"
# `shasum` exists on macOS and ubuntu runners; sha256sum only on Linux.
(cd "$out_dir" && shasum -a 256 "$name" >"$name.sha256")
cat "$out_dir/$name.sha256"
