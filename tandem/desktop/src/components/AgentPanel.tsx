// Agent panel: pick a configured agent and run it against the open PR.
// Several runs can execute at once — each row has its own stop button,
// and clicking a row expands that run's live log (hidden by default).

import {
  Bot,
  Brain,
  Check,
  ChevronDown,
  ChevronRight,
  Clock,
  CornerDownLeft,
  Dot,
  Flag,
  Loader,
  Loader2,
  MessageCircle,
  MessageSquare,
  Play,
  Square,
  Wrench,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { ReactElement } from "react";

import type { LogIcon } from "../lib/agentEvents";
import { summarizeEvent } from "../lib/agentEvents";
import { relativeTime } from "../lib/format";
import { ipc } from "../lib/ipc";
import type { AgentRun, RunEvent } from "../lib/types";
import { useHighlight } from "../state/notifications";
import { loadAgent, saveAgent } from "../state/persist";
import { useAppStore } from "../state/store";
import { Button, Pill, runTone } from "./ui";

const LOG_ICONS: Record<LogIcon, typeof Check> = {
  loader: Loader,
  play: Play,
  check: Check,
  x: X,
  stop: Square,
  clock: Clock,
  comment: MessageSquare,
  chat: MessageCircle,
  brain: Brain,
  wrench: Wrench,
  reply: CornerDownLeft,
  flag: Flag,
  dot: Dot,
};

const isWorking = (r: AgentRun) => r.status === "starting" || r.status === "running";

export function AgentPanel(): ReactElement {
  const specs = useAppStore((s) => s.agentSpecs);
  const runs = useAppStore((s) => s.runs);
  const events = useAppStore((s) => s.agentEvents);
  const startAgentReview = useAppStore((s) => s.startAgentReview);
  const cancelRun = useAppStore((s) => s.cancelRun);
  const setView = useAppStore((s) => s.setView);
  const highlightedRun = useHighlight((s) => s.runId);
  const [selected, setSelected] = useState<string>(loadAgent);
  const [openLog, setOpenLog] = useState<string | null>(null);
  // Spawning the runner takes a beat; without this the button stays
  // live and a second click starts a second agent on the same PR.
  const [starting, setStarting] = useState(false);

  // A jump from the dashboard / bell lands on a run row — open its log
  // so errors are inspectable without an extra click.
  useEffect(() => {
    if (highlightedRun !== null) setOpenLog(highlightedRun);
  }, [highlightedRun]);

  const agentName = selected || (specs[0]?.name ?? "");

  if (specs.length === 0) {
    return (
      <div className="animate-fade-up flex flex-col items-center gap-3 p-6 text-center">
        <Bot size={26} strokeWidth={1.5} className="text-muted" />
        <div className="text-xs leading-relaxed text-muted">
          no agents configured yet — add one in settings (claude, codex, or any custom command)
        </div>
        <Button
          kind="primary"
          onClick={() => {
            setView("settings");
          }}
        >
          open settings
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex items-center gap-2 border-b border-edge p-3">
        <select
          value={agentName}
          onChange={(e) => {
            setSelected(e.target.value);
            saveAgent(e.target.value);
          }}
          className="h-8 flex-1 rounded-lg border border-edge bg-panel-2 px-2 text-xs text-cream"
        >
          {specs.map((spec) => (
            <option key={spec.name} value={spec.name}>
              {spec.name}
            </option>
          ))}
        </select>
        <Button
          kind="primary"
          disabled={!agentName || starting}
          title={starting ? "starting the agent…" : "start a review with this agent"}
          onClick={() => {
            setStarting(true);
            void startAgentReview(agentName).finally(() => {
              setStarting(false);
            });
          }}
        >
          {starting ? (
            <>
              <Loader2 size={11} className="animate-spin" /> starting…
            </>
          ) : (
            <>
              <Play size={11} /> review
            </>
          )}
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="px-3 pb-1 pt-3 text-[11px] uppercase tracking-wide text-muted">runs</div>
        {runs.length === 0 ? (
          <div className="px-3 py-2 text-xs text-muted">none yet</div>
        ) : (
          runs.map((run) => (
            <RunRow
              key={run.run_id}
              run={run}
              flash={highlightedRun === run.run_id}
              logOpen={openLog === run.run_id}
              events={events}
              onToggleLog={() => {
                setOpenLog((cur) => (cur === run.run_id ? null : run.run_id));
              }}
              onStop={() => {
                void cancelRun(run.run_id);
              }}
            />
          ))
        )}
      </div>
    </div>
  );
}

function RunRow({
  run,
  flash,
  logOpen,
  events,
  onToggleLog,
  onStop,
}: {
  run: AgentRun;
  flash: boolean;
  logOpen: boolean;
  events: RunEvent[];
  onToggleLog: () => void;
  onStop: () => void;
}) {
  const Chevron = logOpen ? ChevronDown : ChevronRight;
  return (
    <div className={`border-b border-edge/40 ${flash ? "flash-target" : ""}`}>
      <div className="flex items-center gap-2 px-3 py-2 text-xs">
        <button
          type="button"
          onClick={onToggleLog}
          title={logOpen ? "hide log" : "show this run's log"}
          className="flex min-w-0 flex-1 items-center gap-2 text-left"
        >
          <Chevron size={12} className="shrink-0 text-muted" />
          {isWorking(run) ? <Loader2 size={12} className="shrink-0 animate-spin text-sky" /> : null}
          <span className="truncate text-cream">{run.agent_name}</span>
          <Pill tone={runTone(run.status)}>{run.status.replace("_", " ")}</Pill>
          <Pill tone="muted">
            <MessageSquare size={11} /> {run.comment_count}
          </Pill>
        </button>
        {isWorking(run) ? (
          <button
            type="button"
            onClick={onStop}
            title={`stop ${run.agent_name}`}
            className="inline-flex size-7 shrink-0 items-center justify-center rounded-md text-ember/80 transition-colors hover:bg-ember/10 hover:text-ember"
          >
            <Square size={11} fill="currentColor" />
          </button>
        ) : null}
        <span className="shrink-0 text-[11px] text-muted">{relativeTime(run.started_at)}</span>
      </div>
      {run.error !== null && run.error ? (
        <div className="truncate px-3 pb-2 text-[11px] text-ember" title={run.error}>
          {run.error}
        </div>
      ) : null}
      {logOpen ? (
        <EventLog run={run} liveEvents={events.filter((e) => e.run_id === run.run_id)} />
      ) : null}
    </div>
  );
}

/** Prefers this session's live event stream; falls back to replaying
 * the run's persisted JSONL log (survives restarts, reaches every
 * session — see get_run_log) whenever this session has no live events
 * for it. While the run is still active, keeps polling the persisted
 * log every few seconds so a run opened after it started doesn't just
 * show a frozen snapshot. */
function EventLog({ run, liveEvents }: { run: AgentRun; liveEvents: RunEvent[] }) {
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const [persisted, setPersisted] = useState<RunEvent[] | null>(null);
  const needsPersisted = liveEvents.length === 0;

  useEffect(() => {
    if (!needsPersisted) return;
    let cancelled = false;
    const load = () => {
      ipc
        .getRunLog(run.run_id)
        .then((log) => {
          if (!cancelled) setPersisted(log);
        })
        .catch(() => {
          if (!cancelled) setPersisted((p) => p ?? []);
        });
    };
    load();
    if (!isWorking(run)) return;
    const timer = setInterval(load, 3000);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [needsPersisted, run]);

  const events = needsPersisted ? (persisted ?? []) : liveEvents;
  const lines = events.flatMap((event) =>
    summarizeEvent(event).map((line, i) => ({
      key: `${event.run_id}:${String(event.seq)}:${String(i)}`,
      ...line,
    })),
  );

  useEffect(() => {
    const el = scrollRef.current;
    if (el) el.scrollTop = el.scrollHeight;
  }, [lines.length]);

  if (needsPersisted && persisted === null) {
    return (
      <div className="border-t border-edge/40 bg-ground/70 px-3 py-2 text-[11px] text-muted">
        loading saved log…
      </div>
    );
  }
  if (lines.length === 0) {
    return (
      <div className="border-t border-edge/40 bg-ground/70 px-3 py-2 text-[11px] text-muted">
        no log recorded for this run
      </div>
    );
  }
  return (
    <div
      ref={scrollRef}
      className="max-h-72 overflow-y-auto border-t border-edge/40 bg-ground/70 px-3 py-2 font-mono text-[11px] leading-relaxed"
    >
      {lines.slice(-150).map((line) => {
        const Icon = LOG_ICONS[line.icon];
        return (
          <div
            key={line.key}
            className={`animate-fade-in flex items-start gap-2 py-0.5 ${line.cls}`}
          >
            <Icon size={12} className="mt-0.5 shrink-0 opacity-80" />
            <span className="min-w-0 break-words">{line.text}</span>
          </div>
        );
      })}
    </div>
  );
}
