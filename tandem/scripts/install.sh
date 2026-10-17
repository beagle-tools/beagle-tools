#!/bin/sh
# Tandem installer for macOS and Linux.
#
#   curl -fsSL https://raw.githubusercontent.com/beagle-tools/beagle-tools/main/tandem/scripts/install.sh | sh
#
# Downloads the latest release for this OS/arch and installs it:
#   macOS  -> /Applications/Tandem.app (from the .dmg)
#   Linux  -> .deb via apt/dpkg when available, else AppImage in ~/.local/bin
# After the first install, Tandem updates itself in-app.
set -eu

REPO="beagle-tools/beagle-tools"
BASE="https://github.com/$REPO/releases/download/tandem-latest"

say() { printf '\033[1;36m[tandem]\033[0m %s\n' "$1"; }
fail() {
  printf '\033[1;31m[tandem]\033[0m %s\n' "$1" >&2
  exit 1
}

os=$(uname -s)
arch=$(uname -m)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

case "$os" in
  Darwin)
    case "$arch" in
      arm64) asset="Tandem-macos-arm64.dmg" ;;
      x86_64) asset="Tandem-macos-x64.dmg" ;;
      *) fail "unsupported macOS architecture: $arch" ;;
    esac
    say "downloading $asset…"
    curl -fSL --progress-bar -o "$tmp/Tandem.dmg" "$BASE/$asset"
    say "installing to /Applications…"
    mount=$(hdiutil attach -nobrowse -readonly "$tmp/Tandem.dmg" |
      awk -F'\t' '/\/Volumes\//{print $NF; exit}')
    [ -n "$mount" ] || fail "could not mount the downloaded dmg"
    rm -rf /Applications/Tandem.app
    cp -R "$mount/Tandem.app" /Applications/
    hdiutil detach "$mount" -quiet || true
    # The app isn't notarized yet; clear the quarantine flag so first
    # launch doesn't report it as damaged.
    xattr -dr com.apple.quarantine /Applications/Tandem.app 2>/dev/null || true
    say "done — Tandem is in /Applications. Future updates happen in-app."
    ;;
  Linux)
    if command -v dpkg >/dev/null 2>&1; then
      asset="Tandem-linux-amd64.deb"
      say "downloading $asset…"
      curl -fSL --progress-bar -o "$tmp/tandem.deb" "$BASE/$asset"
      say "installing with apt (may prompt for sudo)…"
      if command -v apt-get >/dev/null 2>&1; then
        sudo apt-get install -y "$tmp/tandem.deb"
      else
        sudo dpkg -i "$tmp/tandem.deb"
      fi
      say "done — launch with: tandem. Future updates happen in-app."
    else
      asset="Tandem-linux-x86_64.AppImage"
      dest="${XDG_DATA_HOME:-$HOME/.local}/bin"
      mkdir -p "$dest"
      say "downloading $asset…"
      curl -fSL --progress-bar -o "$dest/tandem" "$BASE/$asset"
      chmod +x "$dest/tandem"
      say "done — installed to $dest/tandem (make sure it's on your PATH)."
    fi
    ;;
  *)
    fail "unsupported OS: $os — on Windows, run:
  powershell -c \"irm https://raw.githubusercontent.com/$REPO/main/scripts/install.ps1 | iex\""
    ;;
esac
