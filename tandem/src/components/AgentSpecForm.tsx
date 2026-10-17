// The agent-spec edit form (runner, model, prompt, tools, network
// allowlist), split out of AgentEditor.tsx to honor the 400-line rule.

import { useState } from "react";
import type { ReactElement } from "react";

import type { AgentSpec, RunnerKind } from "../lib/types";
import { McpServerPicker } from "./McpServers";
import { Button } from "./ui";

const inputClass =
  "w-full rounded-md border border-edge bg-ground px-2 py-1.5 text-xs text-cream outline-none focus:border-sky";

// Curated per-runner model choices ("" = the CLI's default model).
const MODEL_OPTIONS: Record<
  "claude_headless" | "codex_headless" | "grok_headless",
  [string, string][]
> = {
  claude_headless: [
    ["", "CLI default"],
    ["fable", "Fable 5 — most capable"],
    ["opus", "Opus — latest (5.5)"],
    ["claude-opus-5-5", "Opus 5.5 (pinned)"],
    ["sonnet", "Sonnet 5 — balanced"],
    ["haiku", "Haiku 4.5 — fastest"],
  ],
  codex_headless: [
    ["", "CLI default"],
    ["gpt-5-codex", "GPT-5 Codex"],
    ["gpt-5", "GPT-5"],
    ["gpt-5-mini", "GPT-5 mini — fastest"],
  ],
  grok_headless: [
    ["", "CLI default"],
    ["grok-4.7", "Grok 4.7 — most capable"],
    ["grok-4.6", "Grok 4.6"],
    ["grok-code-fast-1", "Grok Code Fast"],
  ],
};

const CUSTOM_MODEL = "__custom__";

// Default egress allowlists per runner — what the v2 Docker+squid
// sandbox will permit. api.* is the model provider; the second entry
// covers CLI-session auth (token refresh). Unenforced in v1.
const DEFAULT_ALLOWLIST: Record<string, string[]> = {
  claude_headless: ["https://api.anthropic.com", "https://claude.ai"],
  codex_headless: ["https://api.openai.com", "https://chatgpt.com", "https://auth.openai.com"],
  grok_headless: ["https://api.x.ai", "https://accounts.x.ai", "https://x.ai"],
  custom: [],
};

export function defaultAllowlist(kind: string): string[] {
  return DEFAULT_ALLOWLIST[kind] ?? [];
}

function isDefaultAllowlist(list: string[]): boolean {
  const sorted = [...list].sort().join(",");
  return (
    list.length === 0 ||
    Object.values(DEFAULT_ALLOWLIST).some((d) => [...d].sort().join(",") === sorted)
  );
}

function ModelField(props: {
  spec: AgentSpec;
  patch: (p: Partial<AgentSpec>) => void;
}): ReactElement {
  const { spec, patch } = props;
  const kind = spec.runner.kind;

  // Custom runners take any model string.
  if (kind === "custom") {
    return (
      <label className="flex flex-col gap-1">
        <span className="text-muted">model (optional, passed via env)</span>
        <input
          value={spec.model ?? ""}
          onChange={(e) => {
            patch({ model: e.target.value || null });
          }}
          placeholder="default"
          className={inputClass}
        />
      </label>
    );
  }

  const options = MODEL_OPTIONS[kind];
  const known = options.some(([value]) => value === (spec.model ?? ""));
  const selectValue = known ? (spec.model ?? "") : CUSTOM_MODEL;

  return (
    <label className="flex flex-col gap-1">
      <span className="text-muted">model</span>
      <select
        value={selectValue}
        onChange={(e) => {
          const v = e.target.value;
          patch({ model: v === CUSTOM_MODEL ? (spec.model ?? "") : v || null });
        }}
        className={inputClass}
      >
        {options.map(([value, label]) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
        <option value={CUSTOM_MODEL}>custom…</option>
      </select>
      {selectValue === CUSTOM_MODEL ? (
        <input
          value={spec.model ?? ""}
          onChange={(e) => {
            patch({ model: e.target.value || null });
          }}
          placeholder="model id, e.g. claude-fable-5"
          className={`${inputClass} font-mono`}
        />
      ) : null}
    </label>
  );
}

export function SpecForm(props: {
  spec: AgentSpec;
  onSave: (spec: AgentSpec) => void;
  onCancel: () => void;
}): ReactElement {
  const [spec, setSpec] = useState(props.spec);
  const patch = (partial: Partial<AgentSpec>) => {
    setSpec((s) => ({ ...s, ...partial }));
  };

  const runnerKind = spec.runner.kind;
  const setRunner = (kind: string) => {
    const runner: RunnerKind =
      kind === "custom"
        ? { kind: "custom", command: spec.runner.kind === "custom" ? spec.runner.command : "" }
        : kind === "codex_headless"
          ? { kind: "codex_headless" }
          : kind === "grok_headless"
            ? { kind: "grok_headless" }
            : { kind: "claude_headless" };
    // Follow the runner with its default allowlist unless the user has
    // customized it.
    patch(
      isDefaultAllowlist(spec.network_allowlist)
        ? { runner, network_allowlist: defaultAllowlist(kind) }
        : { runner },
    );
  };

  return (
    <div className="mt-3 flex flex-col gap-2.5 rounded-md border border-edge bg-ground/60 p-3 text-xs">
      <label className="flex flex-col gap-1">
        <span className="text-muted">name</span>
        <input
          value={spec.name}
          onChange={(e) => {
            patch({ name: e.target.value });
          }}
          className={inputClass}
        />
      </label>

      <div className="grid grid-cols-2 gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-muted">runner</span>
          <select
            value={runnerKind}
            onChange={(e) => {
              setRunner(e.target.value);
            }}
            className={inputClass}
          >
            <option value="claude_headless">claude (headless)</option>
            <option value="codex_headless">codex (headless)</option>
            <option value="grok_headless">grok (headless)</option>
            <option value="custom">custom command</option>
          </select>
        </label>
        <ModelField spec={spec} patch={patch} />
      </div>

      {spec.runner.kind === "custom" ? (
        <label className="flex flex-col gap-1">
          <span className="text-muted">command (runs via sh -c; prompt arrives on stdin)</span>
          <input
            value={spec.runner.command}
            onChange={(e) => {
              patch({ runner: { kind: "custom", command: e.target.value } });
            }}
            className={`${inputClass} font-mono`}
          />
        </label>
      ) : null}

      <label className="flex flex-col gap-1">
        <span className="text-muted">review instructions</span>
        <textarea
          value={spec.prompt}
          onChange={(e) => {
            patch({ prompt: e.target.value });
          }}
          className={`${inputClass} min-h-24 resize-y`}
        />
      </label>

      <SpecToolsFields spec={spec} patch={patch} />

      <label className="flex flex-col gap-1">
        <span className="text-muted">
          network allowlist (one URL per line — pre-filled with your runner's provider endpoints;
          enforced by the v2 Docker+squid sandbox, see docs/SANDBOXING.md)
        </span>
        <textarea
          value={spec.network_allowlist.join("\n")}
          onChange={(e) => {
            patch({ network_allowlist: e.target.value.split("\n").filter(Boolean) });
          }}
          placeholder="https://api.anthropic.com"
          className={`${inputClass} min-h-12 resize-y font-mono`}
        />
      </label>

      <div className="flex gap-2">
        <Button
          kind="primary"
          onClick={() => {
            props.onSave(spec);
          }}
          disabled={!spec.name.trim()}
        >
          save agent
        </Button>
        <Button onClick={props.onCancel}>cancel</Button>
      </div>
    </div>
  );
}

/** Tool allowlist, timeout, MCP servers (claude only) and the PR-summary
 * toggle — the "what may it do" half of the form. */
function SpecToolsFields({
  spec,
  patch,
}: {
  spec: AgentSpec;
  patch: (p: Partial<AgentSpec>) => void;
}): ReactElement {
  return (
    <>
      <div className="grid grid-cols-2 gap-2">
        <label className="flex flex-col gap-1">
          <span className="text-muted">allowed tools (comma-separated, claude only)</span>
          <input
            value={spec.allowed_tools.join(",")}
            onChange={(e) => {
              patch({
                allowed_tools: e.target.value
                  .split(",")
                  .map((t) => t.trim())
                  .filter(Boolean),
              });
            }}
            placeholder="Read,Grep,Glob"
            className={`${inputClass} font-mono`}
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-muted">timeout (minutes)</span>
          <input
            type="number"
            min={1}
            value={spec.timeout_minutes}
            onChange={(e) => {
              patch({ timeout_minutes: Math.max(1, Number(e.target.value) || 15) });
            }}
            className={inputClass}
          />
        </label>
      </div>

      {spec.runner.kind === "claude_headless" ? (
        <McpServerPicker
          allowed={spec.allowed_tools}
          onChange={(allowed_tools) => {
            patch({ allowed_tools });
          }}
        />
      ) : null}

      <label className="flex cursor-pointer items-start gap-2 rounded-lg border border-edge/60 bg-panel-2/40 px-3 py-2">
        <input
          type="checkbox"
          checked={spec.summary}
          onChange={(e) => {
            patch({ summary: e.target.checked });
          }}
          className="mt-0.5 size-3.5 accent-sky"
        />
        <span className="flex flex-col gap-0.5">
          <span className="font-medium text-cream">PR summary</span>
          <span className="text-muted">
            write a short overview above the diff, revised on each re-review
          </span>
        </span>
      </label>
    </>
  );
}
