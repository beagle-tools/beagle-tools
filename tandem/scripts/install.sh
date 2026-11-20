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
BASE="https://github.com/$REPO/releases/download/beagle-tandem-latest"

say() { printf '\033[1;36m[tandem]\033[0m %s\n' "$1"; }
fail() {
  printf '\033[1;31m[tandem]\033[0m %s\n' "$1" >&2
  exit 1
}

# fetch <asset> <dest>: downloads the asset and checks it against the
# release's <asset>.sha256 before anything installs it.
fetch() {
  curl -fSL --progress-bar -o "$2" "$BASE/$1"
  curl -fsSL -o "$tmp/$1.sha256" "$BASE/$1.sha256" ||
    fail "no checksum published for $1 — refusing to install an unverified download"
  want=$(cut -d' ' -f1 <"$tmp/$1.sha256")
  if command -v sha256sum >/dev/null 2>&1; then
    got=$(sha256sum "$2" | cut -d' ' -f1)
  else
    got=$(shasum -a 256 "$2" | cut -d' ' -f1)
  fi
  [ -n "$want" ] && [ "$want" = "$got" ] || {
    rm -f "$2"
    fail "checksum mismatch for $1 (expected $want, got $got) — download discarded"
  }
  say "checksum ok"
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
    fetch "$asset" "$tmp/Tandem.dmg"
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
      fetch "$asset" "$tmp/tandem.deb"
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
      fetch "$asset" "$tmp/tandem.AppImage"
      mv "$tmp/tandem.AppImage" "$dest/tandem"
      chmod +x "$dest/tandem"
      say "done — installed to $dest/tandem (make sure it's on your PATH)."
    fi
    ;;
  *)
    fail "unsupported OS: $os — on Windows, run:
  powershell -c \"irm https://raw.githubusercontent.com/$REPO/main/tandem/scripts/install.ps1 | iex\""
    ;;
esac
