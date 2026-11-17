#!/usr/bin/env bash
# Per-app release metadata, sourced by the other scripts in this directory.
# The names here are the release contract (docs/RELEASING.md): changing one
# breaks installed updaters, install scripts and the Homebrew tap.
#
#   source scripts/release/apps.sh; app_meta rca
#   -> APP_TITLE APP_PRODUCT APP_MIRROR APP_FORMULA APP_ASSET APP_CASKS
#      APP_CLI_TARGETS
#
# APP_PRODUCT names the desktop installers (`<product>-*`); empty for an
# app without one. APP_ASSET is the CLI tarball prefix
# (`<asset>-<target>.tar.gz`), which is the formula name unless the app
# sets it apart (cli: formula `beagle`, tarballs `beagle-cli-*`).
set -euo pipefail

REPO_URL="https://github.com/beagle-tools/beagle-tools"

app_meta() {
  APP_FORMULA=""
  APP_ASSET=""
  APP_CASKS=""
  APP_CLI_TARGETS=""
  case "${1:?app_meta: missing app}" in
    tandem)
      APP_TITLE="Tandem"
      APP_PRODUCT="Tandem"
      APP_CASKS="beagle-tandem"
      ;;
    rca)
      APP_TITLE="RCA"
      APP_PRODUCT="Beagle"
      APP_FORMULA="beagle-rca"
      APP_CASKS="beagle-rca-desktop"
      APP_CLI_TARGETS="aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl"
      ;;
    workflows)
      APP_TITLE="Workflows"
      APP_PRODUCT="Agentdesk"
      APP_FORMULA="beagle-workflows"
      APP_CASKS="beagle-agentdesk"
      APP_CLI_TARGETS="aarch64-apple-darwin x86_64-apple-darwin"
      ;;
    cli)
      APP_TITLE="CLI"
      APP_PRODUCT=""
      APP_FORMULA="beagle"
      APP_ASSET="beagle-cli"
      APP_CLI_TARGETS="aarch64-apple-darwin x86_64-apple-darwin x86_64-unknown-linux-musl"
      ;;
    *)
      echo "unknown app: $1" >&2
      return 1
      ;;
  esac
  APP_MIRROR="beagle-$1-latest"
  APP_ASSET=${APP_ASSET:-$APP_FORMULA}
}

# Install instructions for the release notes (markdown).
app_install_notes() {
  local app=$1 version=$2
  app_meta "$app"
  local dl="$REPO_URL/releases/download/$APP_MIRROR"
  echo "## Install"
  echo
  case "$app" in
    tandem)
      echo "- macOS: \`brew install --cask beagle-tools/tap/beagle-tandem\`, or the \`.dmg\` below"
      echo "- Windows: \`Tandem-windows-x64-setup.exe\`"
      echo "- Linux: \`Tandem-linux-x86_64.AppImage\` or \`Tandem-linux-amd64.deb\`"
      ;;
    rca)
      echo "- CLI (\`beagle\` + \`beagle-agentd\`): \`brew install beagle-tools/tap/beagle-rca\`,"
      echo "  or a \`beagle-rca-<target>.tar.gz\` below (macOS arm64/x64, Linux x86_64 musl)"
      echo "- Desktop: \`brew install --cask beagle-tools/tap/beagle-rca-desktop\`,"
      echo "  or the \`Beagle-*.dmg\` / \`.AppImage\` / \`.deb\` below"
      echo "- Web: \`docker pull ghcr.io/beagle-tools/beagle-rca-web:$version\`"
      ;;
    workflows)
      echo "- CLIs (\`agentd\`, \`agentctl\`, \`agenttop\`): \`brew install beagle-tools/tap/beagle-workflows\`,"
      echo "  or a \`beagle-workflows-<target>.tar.gz\` below (macOS arm64/x64)"
      echo "- Agentdesk: \`brew install --cask beagle-tools/tap/beagle-agentdesk\`, or the \`.dmg\` below"
      ;;
    cli)
      echo "- \`beagle\` (rca, recap, scout, standup, tandem in one binary): \`brew install beagle-tools/tap/beagle\`,"
      echo "  or a \`beagle-cli-<target>.tar.gz\` below (macOS arm64/x64, Linux x86_64 musl)"
      echo "- Coming from the standalone rca CLI? Its \`beagle-rca\` formula also installs a \`beagle\`:"
      echo "  \`brew uninstall beagle-rca && brew install beagle-tools/tap/beagle\`, then use \`beagle rca …\`"
      echo "- Already installed: \`beagle update\`"
      ;;
  esac
  echo
  if [ -n "$APP_CASKS" ]; then
    echo "Desktop apps already installed update themselves from the in-app update button."
  fi
  echo "Stable download URLs for the newest version: \`$dl/<asset>\`."
}
