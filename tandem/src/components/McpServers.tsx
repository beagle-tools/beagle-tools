// Tick the MCP servers this agent may use. The allowed-tools list stays
// the single source of truth — these checkboxes just write `mcp__<name>`
// into it, so you don't have to know the naming convention.

import { CircleAlert, CircleCheck, Plug, Search } from "lucide-react";
import { useEffect, useState } from "react";

import type { ReactElement, ReactNode } from "react";

import { ipc } from "../lib/ipc";
import type { McpServer } from "../lib/types";
import { Spinner } from "./ui";

/** Allowed either wholesale (`mcp__linear`) or tool by tool
 * (`mcp__linear__get_issue`) — both mean "this agent can use it". */
function isEnabled(allowed: string[], prefix: string): boolean {
  return allowed.some((tool) => tool === prefix || tool.startsWith(`${prefix}__`));
}

export function McpServerPicker({
  allowed,
  onChange,
}: {
  allowed: string[];
  onChange: (next: string[]) => void;
}): ReactElement {
  const [servers, setServers] = useState<McpServer[] | null>(null);
  const [query, setQuery] = useState("");

  useEffect(() => {
    let live = true;
    ipc
      .listMcpServers()
      .then((list) => {
        if (live) setServers(list);
      })
      .catch((e: unknown) => {
        console.warn("couldn't list MCP servers", e);
        if (live) setServers([]);
      });
    return () => {
      live = false;
    };
  }, []);

  const toggle = (server: McpServer) => {
    if (isEnabled(allowed, server.tool_prefix)) {
      // Drop the whole-server entry and any single-tool ones with it.
      onChange(
        allowed.filter(
          (tool) => tool !== server.tool_prefix && !tool.startsWith(`${server.tool_prefix}__`),
        ),
      );
      return;
    }
    onChange([...allowed, server.tool_prefix]);
  };

  const match = query.trim().toLowerCase();
  const visible = (servers ?? []).filter((s) =>
    match === "" ? true : `${s.name} ${s.detail} ${s.tool_prefix}`.toLowerCase().includes(match),
  );
  // Usable servers first — the ones needing a trip to the terminal are
  // not what you came here to pick.
  const byName = (a: McpServer, b: McpServer) => a.name.localeCompare(b.name);
  const ready = visible.filter((s) => s.connected).sort(byName);
  const blocked = visible.filter((s) => !s.connected).sort(byName);

  return (
    <div className="flex flex-col gap-1.5">
      <span className="flex items-center gap-1.5 text-muted">
        <Plug size={11} /> MCP servers this agent may use
      </span>
      {servers === null ? (
        <div className="px-1 py-2">
          <Spinner label="asking claude which servers you have…" />
        </div>
      ) : servers.length === 0 ? (
        <div className="rounded-lg border border-edge/60 bg-panel-2/40 px-3 py-2 text-[11px] text-muted">
          none configured — add one with{" "}
          <span className="font-mono text-cream">claude mcp add</span> in a terminal, then reopen
          this editor
        </div>
      ) : (
        <div className="overflow-hidden rounded-lg border border-edge/60 bg-panel-2/40">
          <div className="flex items-center gap-1.5 border-b border-edge/60 px-2.5 py-1.5">
            <Search size={11} className="shrink-0 text-muted" />
            <input
              value={query}
              onChange={(e) => {
                setQuery(e.target.value);
              }}
              placeholder={`filter ${String(servers.length)} servers…`}
              className="h-5 w-full bg-transparent text-xs text-cream outline-none placeholder:text-muted"
            />
          </div>
          <div className="max-h-56 overflow-y-auto">
            {ready.length > 0 ? (
              <Group label={`ready (${String(ready.length)})`}>
                {ready.map((server) => (
                  <ServerRow
                    key={server.name}
                    server={server}
                    checked={isEnabled(allowed, server.tool_prefix)}
                    onToggle={() => {
                      toggle(server);
                    }}
                  />
                ))}
              </Group>
            ) : null}
            {blocked.length > 0 ? (
              <Group label={`needs authenticating (${String(blocked.length)})`}>
                {blocked.map((server) => (
                  <ServerRow
                    key={server.name}
                    server={server}
                    checked={isEnabled(allowed, server.tool_prefix)}
                    onToggle={() => {
                      toggle(server);
                    }}
                  />
                ))}
              </Group>
            ) : null}
            {visible.length === 0 ? (
              <div className="px-2.5 py-3 text-center text-[11px] text-muted">
                no server matches “{query.trim()}”
              </div>
            ) : null}
          </div>
        </div>
      )}
      <span className="text-[10px] leading-relaxed text-muted">
        Ticking one allows every tool that server offers, writes included. For a read-only agent,
        list the specific tools (e.g. <span className="font-mono">mcp__linear__get_issue</span>) in
        allowed tools above instead.
      </span>
    </div>
  );
}

/** A labelled run of rows, with a divider above it when it isn't first. */
function Group({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="border-b border-edge/60 last:border-b-0">
      <div className="bg-panel/60 px-2.5 py-1 text-[9px] font-medium uppercase tracking-wide text-muted">
        {label}
      </div>
      {children}
    </div>
  );
}

function ServerRow({
  server,
  checked,
  onToggle,
}: {
  server: McpServer;
  checked: boolean;
  onToggle: () => void;
}) {
  return (
    <label className="flex cursor-pointer items-center gap-2 border-b border-edge/40 px-2.5 py-1.5 last:border-b-0 hover:bg-panel-2">
      <input
        type="checkbox"
        checked={checked}
        onChange={onToggle}
        className="size-3.5 shrink-0 accent-sky"
      />
      <span className={`min-w-0 flex-1 ${server.connected ? "" : "opacity-70"}`}>
        <span className="block truncate text-cream">{server.name}</span>
        <span className="block truncate font-mono text-[10px] text-muted">
          {server.tool_prefix} · {server.detail}
        </span>
      </span>
      {server.connected ? (
        <span title="connected — this agent can call it right away" className="shrink-0 text-moss">
          <CircleCheck size={12} />
        </span>
      ) : (
        <span
          // Say what's wrong AND what to do about it: Tandem can't fix
          // this itself, the sign-in is an interactive OAuth flow.
          title={`${server.name} is configured but not signed in, so its tools return nothing. Tandem can't authenticate it for you — run \`claude\` in a terminal, use /mcp to connect it, then reopen this editor.`}
          className="inline-flex shrink-0 items-center gap-1 text-[10px] text-amber"
        >
          <CircleAlert size={12} /> needs auth
        </span>
      )}
    </label>
  );
}
