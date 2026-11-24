// Turn raw agent run events (stream-json envelopes, lifecycle payloads)
// into human-readable log lines for the agent panel.

import type { RunEvent } from "./types";

// Semantic icon keys — the panel maps these to lucide SVG icons.
export type LogIcon =
  | "loader"
  | "play"
  | "check"
  | "x"
  | "stop"
  | "clock"
  | "comment"
  | "chat"
  | "brain"
  | "wrench"
  | "reply"
  | "flag"
  | "dot";

export interface LogLine {
  icon: LogIcon;
  text: string;
  cls: string;
}

function asRecord(v: unknown): Record<string, unknown> | null {
  return typeof v === "object" && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : null;
}

function asArray(v: unknown): unknown[] | null {
  return Array.isArray(v) ? v : null;
}

function str(v: unknown): string | null {
  return typeof v === "string" ? v : null;
}

function num(v: unknown): number | null {
  return typeof v === "number" ? v : null;
}

function parseJson(payload: string): Record<string, unknown> | null {
  try {
    return asRecord(JSON.parse(payload));
  } catch {
    return null;
  }
}

function clip(text: string, max: number): string {
  const oneLine = text.replace(/\s+/g, " ").trim();
  return oneLine.length > max ? `${oneLine.slice(0, max)}…` : oneLine;
}

/** Best-effort human summary; returns [] for noise worth hiding. */
export function summarizeEvent(event: RunEvent): LogLine[] {
  switch (event.kind) {
    case "lifecycle":
      return lifecycleLines(event.payload);
    case "comment":
      return commentLines(event.payload);
    case "runner":
      return runnerLines(event.payload);
    case "raw":
      return event.payload.trim()
        ? [{ icon: "dot", text: clip(event.payload, 200), cls: "text-muted" }]
        : [];
  }
}

function lifecycleLines(payload: string): LogLine[] {
  const obj = parseJson(payload);
  const status = str(obj?.status) ?? "";
  const detail = str(obj?.detail) ?? "";
  const map: Record<string, LogLine> = {
    starting: { icon: "loader", text: `starting ${detail}`, cls: "text-sky" },
    running: { icon: "play", text: "agent process started", cls: "text-sky" },
    succeeded: { icon: "check", text: "run finished", cls: "text-moss" },
    failed: { icon: "x", text: `run failed — ${detail}`, cls: "text-ember" },
    cancelled: { icon: "stop", text: "run cancelled", cls: "text-muted" },
    timed_out: { icon: "clock", text: "run timed out", cls: "text-ember" },
  };
  const line = map[status];
  return line ? [line] : [{ icon: "dot", text: clip(payload, 160), cls: "text-muted" }];
}

function commentLines(payload: string): LogLine[] {
  const obj = parseJson(payload);
  if (!obj) return [];
  const path = str(obj.path) ?? "?";
  const line = num(obj.line) ?? 0;
  const body = str(obj.body) ?? "";
  return [
    {
      icon: "comment",
      text: `${path}:${String(line)} — ${clip(body, 140)}`,
      cls: "text-moss",
    },
  ];
}

function runnerLines(payload: string): LogLine[] {
  const obj = parseJson(payload);
  if (!obj) return [];
  switch (str(obj.type) ?? "") {
    case "system":
    case "rate_limit_event":
      return []; // init / thinking-token / rate-limit telemetry — noise
    case "assistant":
      return assistantLines(obj);
    case "user":
      return toolResultLines(obj);
    case "result":
      return resultLines(obj);
    // Grok Build streaming-json event types.
    case "text": {
      const text = str(obj.text) ?? "";
      return text.trim() && !text.includes('"tandem_comment"')
        ? [{ icon: "chat", text: clip(text, 200), cls: "text-cream/85" }]
        : [];
    }
    case "thought":
      return [{ icon: "brain", text: "thinking", cls: "text-muted italic" }];
    case "tool_call": {
      const name = str(obj.name) ?? str(obj.title) ?? "tool";
      return [{ icon: "wrench", text: name, cls: "text-amber" }];
    }
    case "tool_call_update":
    case "usage":
    case "plan":
    case "available_commands":
      return [];
    case "end":
      return [{ icon: "flag", text: "agent done", cls: "text-sky" }];
    case "error":
      return [
        {
          icon: "x",
          text: clip(str(obj.message) ?? str(obj.error) ?? "agent error", 200),
          cls: "text-ember",
        },
      ];
    default:
      return [{ icon: "dot", text: clip(payload, 160), cls: "text-muted" }];
  }
}

function assistantLines(obj: Record<string, unknown>): LogLine[] {
  const content = asArray(asRecord(obj.message)?.content) ?? [];
  const lines: LogLine[] = [];
  for (const raw of content) {
    const item = asRecord(raw);
    if (!item) continue;
    const type = str(item.type);
    if (type === "text") {
      const text = str(item.text) ?? "";
      // tandem_comment lines already surface as 💬 comment events
      if (text.trim() && !text.includes('"tandem_comment"')) {
        lines.push({ icon: "chat", text: clip(text, 200), cls: "text-cream/85" });
      }
    } else if (type === "thinking") {
      lines.push({ icon: "brain", text: "thinking", cls: "text-muted italic" });
    } else if (type === "tool_use") {
      const name = str(item.name) ?? "tool";
      const input = asRecord(item.input);
      const target =
        str(input?.file_path) ?? str(input?.path) ?? str(input?.command) ?? str(input?.pattern);
      lines.push({
        icon: "wrench",
        text: target ? `${name} — ${clip(target, 120)}` : name,
        cls: "text-amber",
      });
    }
  }
  // collapse consecutive "thinking" spam to one line
  return lines.filter((l, i) => !(l.text === "thinking" && lines[i - 1]?.text === "thinking"));
}

function toolResultLines(obj: Record<string, unknown>): LogLine[] {
  const content = asArray(asRecord(obj.message)?.content) ?? [];
  for (const raw of content) {
    const item = asRecord(raw);
    if (str(item?.type) === "tool_result") {
      const body = str(item?.content);
      return body ? [{ icon: "reply", text: clip(body, 140), cls: "text-muted" }] : [];
    }
  }
  return [];
}

function resultLines(obj: Record<string, unknown>): LogLine[] {
  const seconds = (num(obj.duration_ms) ?? 0) / 1000;
  const cost = num(obj.total_cost_usd);
  const costText = cost !== null ? ` · $${cost.toFixed(2)}` : "";
  return [
    {
      icon: "flag",
      text: `agent done in ${seconds.toFixed(1)}s${costText}`,
      cls: "text-sky",
    },
  ];
}
