// Configure "bring your own AI" reviewers: runner, model, prompt, tools,
// and (for the v2 sandbox) the network allowlist.

import { Bot, Plus } from "lucide-react";
import { useState } from "react";
import type { ReactElement } from "react";

import type { AgentSpec } from "../lib/types";
import { useAppStore } from "../state/store";
import { defaultAllowlist, SpecForm } from "./AgentSpecForm";
import { Button } from "./ui";

const DEFAULT_PROMPT =
  "Review this pull request for correctness bugs, security issues, and " +
  "significant maintainability problems. Prefer few high-signal comments " +
  "over many nits. Use severity `issue` or `blocker` only when confident.";

function newSpec(): AgentSpec {
  return {
    name: "claude-reviewer",
    runner: { kind: "claude_headless" },
    auth: { kind: "cli_session" },
    model: null,
    // Write lets the agent append to its comments drop-box file; the
    // rest are read-only exploration tools.
    allowed_tools: ["Read", "Grep", "Glob", "Write"],
    append_system_prompt: null,
    prompt: DEFAULT_PROMPT,
    env: {},
    network_allowlist: defaultAllowlist("claude_headless"),
    timeout_minutes: 15,
    summary: true,
  };
}

export function AgentEditor(): ReactElement {
  const specs = useAppStore((s) => s.agentSpecs);
  const saveAgentSpec = useAppStore((s) => s.saveAgentSpec);
  const deleteAgentSpec = useAppStore((s) => s.deleteAgentSpec);
  const [editing, setEditing] = useState<AgentSpec | null>(null);

  return (
    <section className="rounded-lg border border-edge bg-panel p-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h2 className="text-sm font-semibold text-cream">Agents</h2>
          <p className="mt-0.5 max-w-[34ch] text-xs text-muted">
            bring your own AI — CLI session, API key, or custom command
          </p>
        </div>
        <Button
          kind="primary"
          onClick={() => {
            setEditing(newSpec());
          }}
        >
          <Plus size={12} /> new agent
        </Button>
      </div>

      <div className="mt-3 flex flex-col gap-2">
        {specs.map((spec) => (
          <div
            key={spec.name}
            className="flex items-center gap-2 rounded-md bg-panel-2 px-2 py-1.5 text-xs"
          >
            <span className="inline-flex items-center gap-1.5 font-medium text-cream">
              <Bot size={13} /> {spec.name}
            </span>
            <span className="text-muted">{spec.runner.kind}</span>
            {spec.model ? <span className="font-mono text-muted">{spec.model}</span> : null}
            <div className="ml-auto flex gap-1.5">
              <Button
                onClick={() => {
                  setEditing(spec);
                }}
              >
                edit
              </Button>
              <Button
                kind="danger"
                onClick={() => {
                  void deleteAgentSpec(spec.name);
                }}
              >
                delete
              </Button>
            </div>
          </div>
        ))}
      </div>

      {editing ? (
        <SpecForm
          spec={editing}
          onSave={(spec) => {
            void saveAgentSpec(spec).then(() => {
              setEditing(null);
            });
          }}
          onCancel={() => {
            setEditing(null);
          }}
        />
      ) : null}
    </section>
  );
}
