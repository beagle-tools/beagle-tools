import { useEffect, useState, type ReactElement } from "react";

import { age } from "../lib/format";
import { errorMessage, ipc } from "../lib/ipc";
import type { AgentNode } from "../lib/types";
import { useStore } from "../state/store";
import { StateBadge } from "./RunList";

// agenttop's AGENTS + CONFIG panes: the DECLARED topology, grouped by
// project, with each agent's last run — true even when nothing has run
// yet, which is the half the lineage forest cannot show.
export function Topology(): ReactElement {
  const agents = useStore((s) => s.agents);
  const loadDag = useStore((s) => s.loadDag);
  const [picked, setPicked] = useState<AgentNode | null>(null);
  const [manifest, setManifest] = useState<string | null>(null);

  useEffect(() => {
    void loadDag();
  }, [loadDag]);

  useEffect(() => {
    if (!picked) {
      setManifest(null);
      return;
    }
    let cancelled = false;
    setManifest(null);
    ipc
      .readManifest(picked.spec.project, picked.spec.name)
      .then((text) => {
        if (!cancelled) setManifest(text);
      })
      .catch((e: unknown) => {
        if (!cancelled) setManifest(`# ${errorMessage(e)}`);
      });
    return () => {
      cancelled = true;
    };
  }, [picked]);

  const projects = [...new Set(agents.map((a) => a.spec.project))];

  return (
    <div className="topology">
      <div className="projects">
        {projects.map((project) => (
          <section key={project}>
            <h3>{project}</h3>
            {agents
              .filter((a) => a.spec.project === project)
              .map((node) => (
                <button
                  key={node.spec.name}
                  className={picked?.spec.name === node.spec.name ? "node selected" : "node"}
                  onClick={() => {
                    setPicked(node);
                  }}
                >
                  <span className="agent">{node.spec.name}</span>
                  <span className="muted small">{node.spec.runner}</span>
                  {node.spec.schedule && (
                    <span className="sched" title="cron schedule">
                      {node.spec.schedule}
                    </span>
                  )}
                  {!node.spec.enabled && <span className="muted small">disabled</span>}
                  {node.latest ? (
                    <>
                      <StateBadge run={node.latest} />
                      <span className="muted small">{age(node.latest.created_at)} ago</span>
                    </>
                  ) : (
                    <span className="muted small">never run</span>
                  )}
                </button>
              ))}
          </section>
        ))}
      </div>

      <div className="config">
        {picked ? (
          <>
            <div className="muted small pathline">{picked.spec.source}</div>
            <pre>{manifest ?? "Reading…"}</pre>
          </>
        ) : (
          <div className="empty muted">Pick an agent to see its manifest.</div>
        )}
      </div>
    </div>
  );
}
