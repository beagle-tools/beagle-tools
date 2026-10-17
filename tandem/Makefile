# Fast, consistent entry points for agents and humans working in this
# repo. Resolves cargo/node/pnpm onto PATH itself — a fresh shell (e.g.
# an agent's sandboxed bash) often doesn't have either on PATH without
# manually sourcing nvm/cargo's env first, so every target here works
# with a bare `make <target>`, no setup step required.
#
# Every recipe line ends in `;` on purpose: macOS ships GNU Make 3.81
# (unmaintained since ~2006), which execs a "simple" command (no shell
# metacharacters) directly via execvp instead of routing it through a
# shell — and that direct exec doesn't see the PATH exported below. The
# trailing `;` forces shell invocation, which does.

NVM_DIR := $(HOME)/.nvm
NODE_BIN_DIR := $(shell ls -d $(NVM_DIR)/versions/node/*/bin 2>/dev/null | sort -V | tail -1)
export PATH := $(HOME)/.cargo/bin:$(NODE_BIN_DIR):$(PATH)
PNPM := corepack pnpm

.PHONY: help install dev dev-bg fmt lint test build check clean

help:
	@echo "make install  - pnpm install"
	@echo "make dev      - run the dev app in the foreground (Tandem Dev, isolated app data)"
	@echo "make dev-bg   - same, backgrounded; logs to .tandem-dev.log"
	@echo "make fmt      - cargo fmt --all + pnpm format (writes)"
	@echo "make lint     - cargo clippy -D warnings + pnpm lint"
	@echo "make test     - cargo test --workspace + pnpm test"
	@echo "make build    - pnpm build (frontend)"
	@echo "make check    - the CI gate for tandem (delegates to the root check-tandem)"
	@echo "make clean    - remove build output (dist/, target/)"

install:
	$(PNPM) install;

# `cargo tauri dev` is always a debug build, which AppDirs::resolve()
# (src-tauri/src/settings.rs) and the window title both key off of to
# keep dev data/identity separate from the installed release app.
dev:
	$(PNPM) tauri dev;

dev-bg:
	nohup $(PNPM) tauri dev > .tandem-dev.log 2>&1 & echo "started — tail -f .tandem-dev.log";

fmt:
	cargo fmt --all;
	$(PNPM) format;

lint:
	cargo clippy --workspace --all-targets --all-features -- -D warnings;
	$(PNPM) lint;

test:
	cargo test --workspace;
	$(PNPM) test;

build:
	$(PNPM) build;

# The monorepo gate owns the real bar (same scripts CI runs); this is
# just a shortcut for running it from inside tandem/.
check:
	$(MAKE) -C .. check-tandem;

clean:
	rm -rf dist target;
