# Sourced by scripts/dev/*.sh. Puts the repo's toolchain on PATH the same
# way the root Makefile does: a fresh shell (an agent's sandboxed bash, a
# new terminal) often has neither cargo nor nvm's node until their init
# scripts run.
# shellcheck shell=bash

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
export REPO_ROOT="$repo_root"

node_bin=$(ls -d "$HOME"/.nvm/versions/node/*/bin 2>/dev/null | sort -V | tail -1 || true)
for bin in "$HOME/.cargo/bin" "$node_bin" /opt/homebrew/bin /usr/local/bin; do
  [ -n "$bin" ] && [ -d "$bin" ] && case ":$PATH:" in
    *":$bin:"*) ;;
    *) PATH="$bin:$PATH" ;;
  esac
done
export PATH

# pnpm through corepack, pinned by each package.json's packageManager.
if command -v pnpm >/dev/null 2>&1; then pnpm=(pnpm); else pnpm=(corepack pnpm); fi

# The pnpm packages (frontends) in the repo.
FRONTENDS=(tandem rca/desktop rca/web workflows/desktop)
