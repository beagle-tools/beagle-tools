#!/usr/bin/env bash
# Copies one platform's Tauri bundles to their stable, version-less names
# (docs/RELEASING.md) and uploads them to the versioned release. The
# publish job later mirrors them onto `beagle-<app>-latest`, whose URLs back
# the Homebrew casks and the install scripts.
#
#   scripts/release/stage-desktop.sh <product> <platform> <cargo-target-dir> <tag>
#
# <platform> is the matrix name: macos-arm, macos-intel, windows or linux.
# Tauri names bundles with the version (Tandem_0.0.2_aarch64.dmg) and puts
# them under <target-dir>[/<triple>]/release/bundle/, so we search for them.
set -euo pipefail

product=${1:?usage: stage-desktop.sh <product> <platform> <target-dir> <tag>}
platform=${2:?missing platform}
target_dir=${3:?missing cargo target dir}
tag=${4:?missing tag}
stage=$(mktemp -d)

# The single bundle matching <glob> under <bundle-subdir>, or fail loudly.
pick() {
  local sub=$1 glob=$2 found="" f
  # Plain globs, not find/globstar: Windows' find.exe and macOS's bash 3.2
  # can't be trusted to be the GNU/bash-4 versions.
  for f in "$target_dir"/release/bundle/"$sub"/$glob "$target_dir"/*/release/bundle/"$sub"/$glob; do
    [ -f "$f" ] && { found=$f; break; }
  done
  if [ -z "$found" ]; then
    echo "no $sub/$glob bundle under $target_dir — did the Tauri build run?" >&2
    exit 1
  fi
  echo "$found"
}

# `src=$(pick …)` on its own line so set -e stops on a missing bundle.
stage_one() {
  local src
  src=$(pick "$1" "$2")
  cp "$src" "$stage/$3"
}

case "$platform" in
  macos-arm) stage_one dmg '*.dmg' "$product-macos-arm64.dmg" ;;
  macos-intel) stage_one dmg '*.dmg' "$product-macos-x64.dmg" ;;
  windows) stage_one nsis '*-setup.exe' "$product-windows-x64-setup.exe" ;;
  linux)
    stage_one appimage '*.AppImage' "$product-linux-x86_64.AppImage"
    stage_one deb '*.deb' "$product-linux-amd64.deb"
    ;;
  *)
    echo "unknown platform: $platform" >&2
    exit 1
    ;;
esac

ls -l "$stage"
gh release upload "$tag" "$stage"/* --clobber
