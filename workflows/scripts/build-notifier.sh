#!/usr/bin/env bash
# Build one branded notifier app per image in assets/notify/*.png (or in
# $AGENTD_NOTIFY_ICONS_DIR — `agentctl daemon install` embeds this script and
# points it at the icons it unpacked, so brew installs need no repo checkout).
# macOS only shows a notification icon that comes from the sending app's bundle, so
# "different icon per pipeline" means "different app per icon". Each gets its
# own bundle id → its own one-time Allow prompt in System Settings.
#
# Output: ~/.local/state/agentd/notifier/<stem>.app  (e.g. agentd.app,
# agentd-fail.app). agentd picks the app whose stem matches the run's icon
# file; drop a png here and re-run to add a brand.
set -euo pipefail

ICONS_DIR="${AGENTD_NOTIFY_ICONS_DIR:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/assets/notify}"
DEST_DIR="${AGENTD_STATE_DIR:-$HOME/.local/state/agentd}/notifier"

SRC="$(brew --prefix terminal-notifier 2>/dev/null)/terminal-notifier.app"
[ -d "$SRC" ] || { echo "terminal-notifier.app not found — brew install terminal-notifier"; exit 1; }

mkdir -p "$DEST_DIR"

# Single temp root for all the intermediate iconsets, cleaned up even if a
# sips/iconutil step fails mid-loop under `set -e` (avoids leaking mktemp dirs).
TMP_ROOT="$(mktemp -d)"
trap 'rm -rf "$TMP_ROOT"' EXIT

for png in "$ICONS_DIR"/*.png; do
  stem="$(basename "$png" .png)"
  app="$DEST_DIR/$stem.app"

  # Rebuilding re-signs the app, which VOIDS its already-granted notification
  # permission — so skip apps whose icon hasn't changed. The hash lives next
  # to the app (inside it would invalidate the signature).
  hash="$(shasum -a 256 "$png" | cut -d' ' -f1)"
  hash_file="$DEST_DIR/.$stem.icon-hash"
  if [ -d "$app" ] && [ "$(cat "$hash_file" 2>/dev/null)" = "$hash" ]; then
    echo "unchanged $app (skipped — keeps its notification permission)"
    continue
  fi

  rm -rf "$app"
  cp -R "$SRC" "$app"

  # PNG → .icns with the standard size set.
  iconset="$(mktemp -d "$TMP_ROOT/iconset.XXXXXX")/icon.iconset"
  mkdir -p "$iconset"
  for size in 16 32 128 256 512; do
    sips -z $size $size "$png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
    double=$((size * 2))
    sips -z $double $double "$png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
  done
  iconutil -c icns "$iconset" -o "$app/Contents/Resources/Terminal.icns"
  rm -rf "$(dirname "$iconset")"   # free per-icon space now; trap is the backstop

  plist="$app/Contents/Info.plist"
  plutil -replace CFBundleIdentifier -string "com.agentd.notifier.$stem" "$plist"
  plutil -replace CFBundleName -string "$stem" "$plist"

  # The edits invalidated the signature; re-sign ad hoc so macOS runs it.
  # Surface (don't swallow) failures — a bad signature silently breaks notifications.
  codesign --force --deep -s - "$app" \
    || echo "warning: codesign failed for $app — notifications may not fire"
  printf '%s' "$hash" > "$hash_file"

  echo "built $app (needs a fresh notification Allow)"
done
echo "each app asks for notification permission once — click Allow"
