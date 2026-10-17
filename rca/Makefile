# Entry points for working on Beagle (rca). The gate itself lives at the repo
# root: `make check` here runs exactly `make check-rca` there, which is what
# CI runs.
#
# Resolves cargo/node onto PATH itself (a fresh or sandboxed shell often has
# neither). Every recipe line ends in `;` on purpose: macOS GNU Make 3.81
# execs "simple" commands without a shell, which would not see this PATH.

NVM_DIR := $(HOME)/.nvm
NODE_BIN_DIR := $(shell ls -d $(NVM_DIR)/versions/node/*/bin 2>/dev/null | sort -V | tail -1)
export PATH := $(HOME)/.cargo/bin:$(NODE_BIN_DIR):$(PATH)
PNPM := corepack pnpm

.PHONY: help check run agentd install fmt lint test desktop web

help:
	@echo "make check    - the CI gate for rca (delegates to the root check-rca)"
	@echo "make run      - open the TUI against ./rcas"
	@echo "make agentd   - build beagle-agentd (put target/debug on PATH or set BEAGLE_AGENTD)"
	@echo "make install  - pnpm install for desktop/ and web/"
	@echo "make fmt      - cargo fmt for both rust workspaces (writes)"
	@echo "make lint     - clippy -D warnings for both rust workspaces"
	@echo "make test     - cargo test for both rust workspaces"
	@echo "make desktop  - run the desktop app (tauri dev)"
	@echo "make web      - run the web site dev server"

check:
	$(MAKE) -C .. check-rca;

run:
	cargo run -p beagle --;

agentd:
	cargo build -p beagle-agentd;

install:
	cd desktop && $(PNPM) install;
	cd web && $(PNPM) install;

fmt:
	cargo fmt --all;
	cd desktop/src-tauri && cargo fmt --all;

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings;
	cd desktop/src-tauri && cargo clippy --workspace --all-targets --all-features -- -D warnings;

test:
	cargo test --workspace --all-features;
	cd desktop/src-tauri && cargo test --workspace --all-features;

desktop:
	cd desktop && $(PNPM) tauri dev;

web:
	cd web && $(PNPM) dev;
