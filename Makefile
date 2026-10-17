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

check: standards check-utils check-tandem check-rca check-workflows

standards:
	./scripts/check-file-length.sh;
	node scripts/check-standards.mjs;

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
