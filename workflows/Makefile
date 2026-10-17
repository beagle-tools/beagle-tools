.PHONY: build check fmt clippy test install restart-daemon clean

build:
	cargo build --workspace

# The full gate (crates + desktop, exactly what CI runs) lives at the repo root.
check:
	$(MAKE) -C .. check-workflows;

fmt:
	cargo fmt --all --check

clippy:
	cargo clippy --workspace --all-targets -- -D warnings

test:
	cargo test --workspace

# Installing without restarting the daemon leaves the OLD agentd reconciling
# manifests. Because manifests use serde(deny_unknown_fields), any manifest
# relying on a field the running binary predates is rejected and its agent is
# dropped from the store — silently, on the next tick. The symptom is a stage
# that "disappears" mid-chain ("agent not found in the store"), which looks
# nothing like a version mismatch. So the restart is part of installing.
install:
	cargo install --path crates/agentd
	cargo install --path crates/agentctl
	cargo install --path crates/agenttop
	@$(MAKE) --no-print-directory restart-daemon

# Two kickstarts, 12s apart, before giving up. One attempt was still too few:
# with `cargo install` hammering the disk and two chains running, launchd
# occasionally swallows the first one and the check cried wolf on a daemon
# that was merely slow. Retrying beats a longer timeout — it also recovers
# from a dropped kickstart, which waiting never does.
#
# Restarting is fiddlier than it looks. The plist's KeepAlive only revives a
# CRASHED process, so any clean exit — including the TERM below — stays down
# unless we start it again ourselves. And kickstart races a still-exiting
# process: it can return 0 while the pending signal kills what it just
# started. So: signal, WAIT for the pid to go, start, then verify a pid
# exists. Reporting success off kickstart's exit code is how this silently
# left the daemon down and every later run sat Pending.
#
# Restarting mid-run does not kill the run (its tmux session is independent),
# but the monitor keeps log offsets in memory and resumes from the current end
# of file — so events written during the gap never reach the event feed, and
# that run's log in agenttop has a silent hole. Warn rather than refuse: this
# is sometimes exactly what you want (the running binary is the broken one).
restart-daemon:
	@active=$$(agentctl get runs --state running -n 50 2>/dev/null | grep -c running || true); \
	if [ "$${active:-0}" -gt 0 ]; then \
		echo "  WARNING: $$active run(s) still going — their agenttop logs will have a gap"; \
		echo "           (the runs themselves survive; check with: agentctl get runs --state running)"; \
	fi
	@if launchctl list 2>/dev/null | grep -q com.agentd.daemon; then \
		launchctl kill TERM gui/$$(id -u)/com.agentd.daemon 2>/dev/null || true; \
		for i in 1 2 3 4 5 6 7 8 9 10; do \
			pgrep -qf '[c]argo/bin/agentd' || break; sleep 1; \
		done; \
		for attempt in 1 2; do \
			launchctl kickstart gui/$$(id -u)/com.agentd.daemon >/dev/null 2>&1 || true; \
			for i in 1 2 3 4 5 6 7 8 9 10 11 12; do \
				sleep 1; pgrep -qf '[c]argo/bin/agentd' && break; \
			done; \
			pgrep -qf '[c]argo/bin/agentd' && break; \
		done; \
		if pgrep -qf '[c]argo/bin/agentd'; then \
			echo "  agentd restarted (pid $$(pgrep -f '[c]argo/bin/agentd'))"; \
		else \
			echo "  ERROR: agentd did NOT come back up. Runs will sit Pending."; \
			echo "         launchctl kickstart gui/$$(id -u)/com.agentd.daemon"; \
			echo "         tail ~/Library/Logs/agentd/agentd.err.log"; \
			exit 1; \
		fi; \
	else \
		echo "  agentd is not managed by launchd here — restart it yourself if it is running"; \
	fi
