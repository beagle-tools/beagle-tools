#!/usr/bin/env bash
# Install or verify everything needed to build, test and run every app on
# macOS. Idempotent: each step says what it checks and skips what is
# already there. See docs/DEVELOPMENT.md.
#
#   scripts/dev/bootstrap.sh            install what's missing
#   scripts/dev/bootstrap.sh --check    report only; exits 1 if anything is missing
#   scripts/dev/bootstrap.sh --with-optional   also install optional tools (glow)
set -euo pipefail
# shellcheck source=scripts/dev/env.sh
source "$(dirname "$0")/env.sh"

check_only=0
optional=0
for arg in "$@"; do
  case "$arg" in
    --check) check_only=1 ;;
    --with-optional) optional=1 ;;
    -h | --help) sed -n '2,8p' "$0"; exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

missing=0
ok() { printf '  ok       %s\n' "$*"; }
miss() { printf '  MISSING  %s\n' "$*"; missing=$((missing + 1)); }
info() { printf '  ...      %s\n' "$*"; }
step() { printf '\n==> %s\n' "$*"; }
# do_install <description> <command...>: runs it unless --check.
do_install() {
  local what=$1
  shift
  if [ "$check_only" -eq 1 ]; then
    miss "$what (run without --check to install)"
    return 1
  fi
  info "installing $what: $*"
  "$@"
}

step "platform"
if [ "$(uname -s)" = Darwin ]; then
  ok "macOS $(sw_vers -productVersion) ($(uname -m))"
else
  info "not macOS: this script targets macOS; install the equivalents by hand (docs/DEVELOPMENT.md)"
fi

step "Xcode Command Line Tools (C toolchain for Rust and Tauri)"
if xcode-select -p >/dev/null 2>&1; then
  ok "$(xcode-select -p)"
elif [ "$check_only" -eq 1 ]; then
  miss "Xcode Command Line Tools"
else
  info "running xcode-select --install (a GUI installer opens; re-run bootstrap when it finishes)"
  xcode-select --install || true
  missing=$((missing + 1))
fi

step "Homebrew"
if command -v brew >/dev/null 2>&1; then
  ok "$(brew --version | head -1)"
  have_brew=1
else
  miss "Homebrew — install it from https://brew.sh, then re-run this script"
  have_brew=0
fi

# brew_tool <command> <formula> [optional]
brew_tool() {
  local cmd=$1 formula=$2
  if command -v "$cmd" >/dev/null 2>&1; then
    ok "$cmd ($(command -v "$cmd"))"
  elif [ "$have_brew" -eq 0 ]; then
    miss "$cmd (needs Homebrew)"
  else
    do_install "$formula" brew install "$formula" || true
  fi
}

step "CLI tools (gh, tmux, jq)"
brew_tool gh gh          # issues/PRs, tandem's GitHub auth
brew_tool tmux tmux      # workflows' agentd runs jobs in tmux sessions
brew_tool jq jq          # scripts and agents read JSON with it

step "optional tools"
if command -v glow >/dev/null 2>&1; then
  ok "glow ($(command -v glow))"
elif [ "$optional" -eq 1 ] && [ "$have_brew" -eq 1 ]; then
  do_install glow brew install glow || true
else
  info "glow not installed (optional markdown viewer; --with-optional installs it)"
fi

step "Rust (rustup + the toolchain pinned in rust-toolchain.toml)"
toolchain=$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\(.*\)"/\1/p' "$REPO_ROOT/rust-toolchain.toml")
if ! command -v rustup >/dev/null 2>&1; then
  if [ "$have_brew" -eq 0 ]; then
    miss "rustup (needs Homebrew)"
  elif do_install rustup brew install rustup; then
    # Homebrew's rustup is keg-only; rustup-init sets up ~/.cargo/bin proxies,
    # which is where the Makefile and these scripts look.
    prefix=$(brew --prefix rustup)
    PATH="$HOME/.cargo/bin:$prefix/bin:$PATH"
    if [ -x "$prefix/bin/rustup-init" ]; then
      "$prefix/bin/rustup-init" -y --no-modify-path --default-toolchain none
    fi
  fi
fi
if command -v rustup >/dev/null 2>&1; then
  ok "rustup ($(command -v rustup))"
  if rustup toolchain list | grep -q "^$toolchain"; then
    ok "toolchain $toolchain"
  else
    (cd "$REPO_ROOT" && do_install "toolchain $toolchain" rustup toolchain install) || true
  fi
fi

step "Node (version pinned in .nvmrc, via nvm) + corepack/pnpm"
want_node=$(tr -d '[:space:]v' <"$REPO_ROOT/.nvmrc")
export NVM_DIR="$HOME/.nvm"
nvm_sh=""
for candidate in "$NVM_DIR/nvm.sh" "/opt/homebrew/opt/nvm/nvm.sh" "/usr/local/opt/nvm/nvm.sh"; do
  [ -s "$candidate" ] && { nvm_sh=$candidate; break; }
done
node_major() { node --version 2>/dev/null | sed 's/^v\([0-9]*\).*/\1/'; }
if [ "$(node_major)" = "${want_node%%.*}" ]; then
  ok "node $(node --version) ($(command -v node))"
elif [ "$check_only" -eq 1 ]; then
  miss "node $want_node (found: $(node --version 2>/dev/null || echo none); nvm: ${nvm_sh:-not installed})"
else
  if [ -z "$nvm_sh" ] && [ "$have_brew" -eq 1 ]; then
    do_install nvm brew install nvm && mkdir -p "$NVM_DIR" && nvm_sh="$(brew --prefix nvm)/nvm.sh"
  fi
  if [ -z "$nvm_sh" ]; then
    miss "nvm (needs Homebrew, or install from github.com/nvm-sh/nvm)"
  else
    info "installing node $want_node with nvm"
    # nvm's scripts aren't written for `set -u`.
    set +u
    # shellcheck source=/dev/null
    . "$nvm_sh"
    (cd "$REPO_ROOT" && nvm install)
    set -u
    PATH="$(ls -d "$NVM_DIR"/versions/node/*/bin | sort -V | tail -1):$PATH"
    ok "node $(node --version)"
  fi
fi
if command -v corepack >/dev/null 2>&1; then
  pnpm_bin="$(dirname "$(command -v corepack)")/pnpm"
  if [ -e "$pnpm_bin" ]; then
    ok "corepack enabled ($pnpm_bin)"
  else
    do_install "corepack shims" corepack enable || true
  fi
  if version=$(cd "$REPO_ROOT/tandem" && COREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack pnpm --version 2>/dev/null); then
    ok "pnpm $version (pinned by packageManager)"
  else
    miss "pnpm via corepack (try: cd tandem && corepack pnpm --version)"
  fi
else
  miss "corepack (ships with node $want_node)"
fi

step "frontend dependencies + Playwright Chromium"
if ! command -v node >/dev/null 2>&1; then
  miss "node is required for this step"
else
  pnpm=(env COREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack pnpm)
  for app in "${FRONTENDS[@]}"; do
    dir="$REPO_ROOT/$app"
    if [ -d "$dir/node_modules" ]; then
      ok "$app: node_modules present"
    else
      (cd "$dir" && do_install "$app dependencies" "${pnpm[@]}" install --frozen-lockfile) ||
        continue
    fi
    if ! grep -q '"@playwright/test"' "$dir/package.json"; then
      info "$app: no @playwright/test dependency yet — skipping browser install"
      continue
    fi
    # --dry-run prints where this Playwright version keeps its Chromium.
    location=$(cd "$dir" && "${pnpm[@]}" exec playwright install --dry-run chromium 2>/dev/null |
      sed -n 's/.*Install location:[[:space:]]*//p' | head -1)
    if [ -n "$location" ] && [ -d "$location" ]; then
      ok "$app: Playwright Chromium ($location)"
    else
      (cd "$dir" && do_install "$app Playwright Chromium" "${pnpm[@]}" exec playwright install chromium) || true
    fi
  done
fi

echo ""
if [ "$missing" -gt 0 ]; then
  echo "$missing item(s) missing.$([ "$check_only" -eq 1 ] && echo ' Run make bootstrap to install them.')"
  exit 1
fi
echo "OK: toolchain ready — try: make dev APP=tandem (see docs/DEVELOPMENT.md)"
