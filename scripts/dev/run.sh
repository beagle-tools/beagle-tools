#!/usr/bin/env bash
# Start one app locally, in the foreground (Ctrl-C stops it). Humans and
# agents use the same entry point; see docs/DEVELOPMENT.md.
#
#   scripts/dev/run.sh <target> [--browser] [-- <args passed to the app>]
#   scripts/dev/run.sh --list
#
# --browser (Tauri targets only) runs just the frontend's vite dev server in
# mode `e2e`, where the app installs its mockIPC fixtures instead of talking
# to Rust — the UI works in any browser, so Playwright can drive it.
set -euo pipefail
# shellcheck source=scripts/dev/env.sh
source "$(dirname "$0")/env.sh"

# target | kind | dir | what it starts
TARGETS="\
tandem|tauri|tandem|Tandem desktop app (tauri dev, port 1420)
rca-desktop|tauri|rca/desktop|Beagle RCA desktop app (tauri dev, port 1420)
workflows-desktop|tauri|workflows/desktop|Agentdesk desktop app (tauri dev, port 1421)
rca-tui|cargo:beagle|rca|beagle TUI/CLI (cargo run -p beagle)
rca-agentd|cargo:beagle-agentd|rca|beagle-agentd daemon, foreground
rca-web|web|rca/web|RCA static site (astro dev)
agentd|cargo:agentd|workflows|agentd scheduler daemon, foreground
agenttop|cargo:agenttop|workflows|agenttop TUI
agentctl|cargo:agentctl|workflows|agentctl CLI"

usage() {
  echo "usage: scripts/dev/run.sh <target> [--browser] [-- <app args>]"
  echo ""
  echo "targets:"
  while IFS='|' read -r name kind _ desc; do
    extra=""
    [ "$kind" = tauri ] && extra=" [--browser: frontend only, mocked IPC]"
    printf '  %-18s %s%s\n' "$name" "$desc" "$extra"
  done <<<"$TARGETS"
}

target=""
browser=0
app_args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --list | -h | --help) usage; exit 0 ;;
    --browser) browser=1 ;;
    --) shift; app_args=("$@"); break ;;
    -*) echo "unknown flag: $1" >&2; usage >&2; exit 2 ;;
    *) [ -z "$target" ] || { echo "one target at a time" >&2; exit 2; }; target=$1 ;;
  esac
  shift
done
[ -n "$target" ] || { usage >&2; exit 2; }

line=$(grep "^$target|" <<<"$TARGETS" || true)
[ -n "$line" ] || { echo "unknown target: $target" >&2; usage >&2; exit 2; }
IFS='|' read -r _ kind dir desc <<<"$line"
[ -d "$REPO_ROOT/$dir" ] || { echo "$dir does not exist in this checkout" >&2; exit 1; }
cd "$REPO_ROOT/$dir"

if [ "$browser" -eq 1 ] && [ "$kind" != tauri ]; then
  echo "--browser only applies to Tauri targets ($target already runs without Tauri)" >&2
  exit 2
fi

# Installs JS deps on first run; a no-op afterwards.
ensure_node_modules() {
  if [ ! -d node_modules ]; then
    echo "==> [$dir] installing dependencies (first run)"
    "${pnpm[@]}" install --frozen-lockfile
  fi
}

# Bash 3.2 (macOS) treats "${arr[@]}" of an empty array as unbound under -u.
args=(${app_args[@]+"${app_args[@]}"})

case "$kind" in
  tauri)
    ensure_node_modules
    if [ "$browser" -eq 1 ]; then
      echo "==> [$target] frontend only, mocked IPC (vite --mode e2e)"
      echo "    open the URL vite prints below; drive it with Playwright or a browser"
      exec "${pnpm[@]}" exec vite --mode e2e ${args[@]+"${args[@]}"}
    fi
    echo "==> [$target] $desc"
    exec "${pnpm[@]}" tauri dev ${args[@]+"${args[@]}"}
    ;;
  web)
    ensure_node_modules
    echo "==> [$target] $desc"
    exec "${pnpm[@]}" dev ${args[@]+"${args[@]}"}
    ;;
  cargo:*)
    pkg=${kind#cargo:}
    echo "==> [$target] $desc"
    exec cargo run --locked -p "$pkg" -- ${args[@]+"${args[@]}"}
    ;;
esac
