#!/usr/bin/env bash
# Writes one app's Homebrew files into a checkout of beagle-tools/homebrew-tap:
# renders its formula from packaging/homebrew/Formula/<name>.rb.tmpl (version
# + per-target sha256) and copies its casks verbatim. Committing and pushing
# is the workflow's job, so this runs locally too.
#
#   scripts/release/homebrew.sh <app> <version> <tap-dir> [<sha-dir>]
#
# <sha-dir> holds `<formula>-<target>.tar.gz.sha256` files. When omitted the
# script downloads them from the `beagle-<app>-v<version>` release (needs
# GH_TOKEN). Local check with fake values:
#   mkdir -p /tmp/shas && for t in aarch64-apple-darwin x86_64-apple-darwin \
#     x86_64-unknown-linux-musl; do echo "$(printf 'a%.0s' {1..64})  x" \
#     > /tmp/shas/beagle-rca-$t.tar.gz.sha256; done
#   scripts/release/homebrew.sh rca 1.2.3 /tmp/tap /tmp/shas
set -euo pipefail

app=${1:?usage: homebrew.sh <app> <version> <tap-dir> [<sha-dir>]}
version=${2:?missing version}
tap=${3:?missing tap dir}
sha_dir=${4:-}
here="$(cd "$(dirname "$0")" && pwd)"
root="$(cd "$here/../.." && pwd)"
# shellcheck source=apps.sh
source "$here/apps.sh"
app_meta "$app"

[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "bad version: $version" >&2; exit 1; }
mkdir -p "$tap/Formula" "$tap/Casks"

if [ -n "$APP_FORMULA" ]; then
  if [ -z "$sha_dir" ]; then
    sha_dir=$(mktemp -d)
    gh release download "beagle-$app-v$version" --repo beagle-tools/beagle-tools \
      --dir "$sha_dir" --pattern "$APP_FORMULA-*.tar.gz.sha256"
  fi
  args=(-e "s|@VERSION@|$version|g")
  for target in $APP_CLI_TARGETS; do
    file="$sha_dir/$APP_FORMULA-$target.tar.gz.sha256"
    [ -f "$file" ] || { echo "missing $file" >&2; exit 1; }
    sha=$(awk '{print $1; exit}' "$file")
    [[ "$sha" =~ ^[0-9a-f]{64}$ ]] || { echo "bad sha256 in $file: $sha" >&2; exit 1; }
    token=$(echo "$target" | tr '[:lower:]-' '[:upper:]_')
    args+=(-e "s|@SHA_${token}@|$sha|g")
  done
  out="$tap/Formula/$APP_FORMULA.rb"
  sed "${args[@]}" "$root/packaging/homebrew/Formula/$APP_FORMULA.rb.tmpl" >"$out"
  if grep -n '@[A-Z0-9_]*@' "$out"; then
    echo "unrendered placeholders left in $out" >&2
    exit 1
  fi
  echo "rendered $out"
fi

for cask in $APP_CASKS; do
  cp "$root/packaging/homebrew/Casks/$cask.rb" "$tap/Casks/$cask.rb"
  echo "copied Casks/$cask.rb"
done
