# Entry points for Agentdesk. Resolves cargo and node onto PATH itself —
# a fresh shell (an agent's, or a `make` run that never sourced nvm) has
# neither, and every target here has to work from a bare `make <target>`.
#
# Every recipe line ends in `;` on purpose: macOS ships GNU Make 3.81,
# which execs a "simple" command directly via execvp instead of through a
# shell — and that direct exec does not see the PATH exported below. The
# trailing `;` forces a shell, which does.

NVM_DIR := $(HOME)/.nvm
NODE_BIN_DIR := $(shell ls -d $(NVM_DIR)/versions/node/*/bin 2>/dev/null | sort -V | tail -1)
export PATH := $(HOME)/.cargo/bin:$(NODE_BIN_DIR):$(PATH)
PNPM := corepack pnpm

.PHONY: help install dev dev-bg fmt lint test build check clean

help:
	@echo "make install  - pnpm install";
	@echo "make dev      - run the app in the foreground (Agentdesk Dev)";
	@echo "make dev-bg   - same, backgrounded; logs to .agentdesk-dev.log";
	@echo "make fmt      - cargo fmt --all + pnpm format (writes)";
	@echo "make lint     - cargo clippy -D warnings + pnpm lint";
	@echo "make test     - cargo test --workspace + pnpm test";
	@echo "make build    - release bundle (.app + .dmg)";
	@echo "make check    - the full workflows gate CI runs (delegates to repo root)";

install:
	$(PNPM) install;

# Always a debug build, which titles the window "Agentdesk Dev". It reads
# the SAME store as the release app — there is only one agentd — so the
# title is the only thing telling two open windows apart.
dev:
	$(PNPM) tauri dev;

dev-bg:
	nohup $(PNPM) tauri dev > .agentdesk-dev.log 2>&1 & echo "started — tail -f .agentdesk-dev.log";

fmt:
	cargo fmt --all;
	$(PNPM) format;

lint:
	cargo clippy --workspace --all-targets -- -D warnings;
	$(PNPM) lint;

test:
	cargo test --workspace;
	$(PNPM) test;

build:
	$(PNPM) tauri build;

check:
	$(MAKE) -C ../.. check-workflows;

clean:
	rm -rf dist target node_modules;
