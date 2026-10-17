# Monorepo gate. `make check` runs exactly what CI runs — same scripts, same
# order — so a green local run is a green CI run. Per-app targets exist so
# you only pay for the app you touched.
#
# Resolves cargo/node onto PATH itself: a fresh shell (e.g. an agent's
# sandboxed bash) often has neither without sourcing nvm/cargo first.
# Every recipe line ends in `;` on purpose: macOS's GNU Make 3.81 execs a
# "simple" command directly (no shell), which doesn't see the PATH below.

NVM_DIR := $(HOME)/.nvm
NODE_BIN_DIR := $(shell ls -d $(NVM_DIR)/versions/node/*/bin 2>/dev/null | sort -V | tail -1)
export PATH := $(HOME)/.cargo/bin:$(NODE_BIN_DIR):$(PATH)

.PHONY: help check standards check-utils check-tandem check-rca check-workflows

help:
	@echo "make check            - every gate CI runs (standards + all apps)"
	@echo "make standards        - 400-line rule + shared-config drift check"
	@echo "make check-utils      - shared Rust crate"
	@echo "make check-tandem     - tandem (rust + frontend)"
	@echo "make check-rca        - rca (cli/crates, desktop rust + frontend, web)"
	@echo "make check-workflows  - workflows (crates, desktop rust + frontend)"
	@echo ""
	@echo "make test-affected [BASE=origin/main] - cargo tests for packages changed since BASE (+ dependents)"
	@echo "make test-scripts     - unit tests for the CI scripts (test selection)"
	@echo "make e2e-<app>        - Playwright e2e: tandem, rca-desktop, rca-web, workflows-desktop"
	@echo ""
	@echo "make bootstrap        - install/verify the local toolchain on macOS (idempotent)"
	@echo "make bootstrap-check  - report what's missing, change nothing"
	@echo "make dev APP=<target> [BROWSER=1] [ARGS=...]  - start an app (or make dev-<target>)"
	@echo "                        targets: ./scripts/dev/run.sh --list"

check: standards check-utils check-tandem check-rca check-workflows

standards:
	./scripts/check-file-length.sh;
	node scripts/check-standards.mjs;
	node --test scripts/ci/*.test.mjs;

check-utils:
	./scripts/ci/rust.sh utils;

check-tandem:
	./scripts/ci/rust.sh tandem;
	./scripts/ci/frontend.sh tandem;

check-rca:
	./scripts/ci/rust.sh rca;
	./scripts/ci/rust.sh rca/desktop/src-tauri;
	./scripts/ci/frontend.sh rca/desktop;
	./scripts/ci/frontend.sh rca/web;

check-workflows:
	./scripts/ci/rust.sh workflows;
	./scripts/ci/rust.sh workflows/desktop;
	./scripts/ci/frontend.sh workflows/desktop;

# ---- tests ----------------------------------------------------------------
# Same selection CI applies to pull requests (docs/TESTING.md). Includes
# uncommitted and untracked files, so it works before you commit.
BASE ?= origin/main
RUST_WORKSPACES := utils tandem rca rca/desktop/src-tauri workflows workflows/desktop

.PHONY: test-affected test-scripts e2e e2e-tandem e2e-rca-desktop e2e-rca-web e2e-workflows-desktop

test-affected:
	@set -e; for ws in $(RUST_WORKSPACES); do \
	  AFFECTED_BASE=$(BASE) ./scripts/ci/rust.sh $$ws --tests-only; \
	done

test-scripts:
	node --test scripts/ci/*.test.mjs;

e2e: e2e-tandem e2e-rca-desktop e2e-rca-web e2e-workflows-desktop

e2e-tandem:
	cd tandem && corepack pnpm e2e;
e2e-rca-desktop:
	cd rca/desktop && corepack pnpm e2e;
e2e-rca-web:
	cd rca/web && corepack pnpm e2e;
e2e-workflows-desktop:
	cd workflows/desktop && corepack pnpm e2e;

# ---- local dev (docs/DEVELOPMENT.md) --------------------------------------
.PHONY: bootstrap bootstrap-check dev

bootstrap:
	./scripts/dev/bootstrap.sh;

bootstrap-check:
	./scripts/dev/bootstrap.sh --check;

dev:
	@test -n "$(APP)" || { echo "usage: make dev APP=<target> [BROWSER=1] [ARGS=...]"; ./scripts/dev/run.sh --list; exit 2; }
	./scripts/dev/run.sh $(APP) $(if $(BROWSER),--browser) $(if $(ARGS),-- $(ARGS));

dev-%:
	./scripts/dev/run.sh $* $(if $(BROWSER),--browser) $(if $(ARGS),-- $(ARGS));
