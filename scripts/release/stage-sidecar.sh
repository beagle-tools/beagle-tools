#!/usr/bin/env bash
# Builds a CLI that ships inside a Tauri app bundle (a Tauri "sidecar") for
# this job's target and declares it in the app's tauri.conf.json, so the
# bundle that tauri-action builds next carries it.
#
#   scripts/release/stage-sidecar.sh <project> <cargo-dir> <package> <bin> [tauri build args]
#
# e.g. stage-sidecar.sh tandem tandem tandem-cli tandem-cli "--target aarch64-apple-darwin"
#
# The declaration (bundle.externalBin) is added here, in release CI only,
# never committed: Tauri's build script refuses to compile when a declared
# sidecar is missing, which would make every plain `cargo clippy`/`cargo
# test` of the app depend on building the CLI first. Like the version stamp,
# the edited tauri.conf.json is thrown away with the runner.
set -euo pipefail

project=${1:?usage: stage-sidecar.sh <project> <cargo-dir> <package> <bin> [tauri build args]}
cargo_dir=${2:?missing cargo dir}
package=${3:?missing cargo package}
bin=${4:?missing binary name}
args=${5:-}

# The triple Tauri will bundle for: --target from the build args, else host.
if [[ "$args" == *--target* ]]; then
  triple=${args##*--target }
  triple=${triple%% *}
  target_args=(--target "$triple")
  built_dir="$cargo_dir/target/$triple/release"
else
  triple=$(rustc -vV | sed -n 's/^host: //p')
  target_args=()
  built_dir="$cargo_dir/target/release"
fi
ext=""
[[ "$triple" == *windows* ]] && ext=".exe"

(cd "$cargo_dir" && cargo build --release --locked -p "$package" "${target_args[@]}")

# Tauri looks for <externalBin>-<target triple>[.exe] and installs it as
# plain <bin> beside the app's executable.
dest="$project/src-tauri/binaries"
mkdir -p "$dest"
cp "$built_dir/$bin$ext" "$dest/$bin-$triple$ext"
echo "staged $dest/$bin-$triple$ext"

node - "$project/src-tauri/tauri.conf.json" "binaries/$bin" <<'JS'
const { readFileSync, writeFileSync } = require("node:fs");
const [path, entry] = process.argv.slice(2);
const conf = JSON.parse(readFileSync(path, "utf8"));
conf.bundle ??= {};
conf.bundle.externalBin = [...new Set([...(conf.bundle.externalBin ?? []), entry])];
writeFileSync(path, JSON.stringify(conf, null, 2) + "\n");
console.log(`${path}: bundle.externalBin = ${JSON.stringify(conf.bundle.externalBin)}`);
JS
