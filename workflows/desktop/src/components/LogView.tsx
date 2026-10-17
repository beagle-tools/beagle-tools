import { Fragment, useEffect, useMemo, useRef, type ReactNode, type ReactElement } from "react";

import type { TranscriptEntry } from "../lib/types";
import { useStore } from "../state/store";

// A run's transcript, rendered from the same entries agenttop draws. A
// model run reads as a conversation — what it said, what it ran, what came
// back, what it was refused. A script run reads as a log — levelled lines
// with the source that wrote them.
export function LogView(): ReactElement {
  const log = useStore((s) => s.log);
  const filter = useStore((s) => s.logFilter);
  const setFilter = useStore((s) => s.setLogFilter);
  const bottom = useRef<HTMLDivElement>(null);

  const needle = filter.trim().toLowerCase();
  const shown = useMemo(() => {
    if (!log) return [];
    if (!needle) return log.entries;
    // Matched against the whole line as drawn — tag, source and body — so
    // searching `error` or `gather` finds what the eye would.
    return log.entries.filter((e) =>
      `${e.tag} ${e.source ?? ""} ${e.body}`.toLowerCase().includes(needle),
    );
  }, [log, needle]);

  // Follow the tail, but only unfiltered — jumping to the bottom of a
  // filtered view fights the person reading a match.
  useEffect(() => {
    if (!needle) bottom.current?.scrollIntoView({ block: "end" });
  }, [shown, needle]);

  if (!log) return <div className="empty muted">Select a step.</div>;
  if (log.missing_reason) return <div className="empty muted">{log.missing_reason}</div>;

  return (
    <div className="logwrap">
      <div className="logbar">
        <span className={`kind ${log.agent ? "agent" : "script"}`}>
          {log.agent ? "conversation" : "script log"}
        </span>
        <input
          type="search"
          placeholder="Search…"
          value={filter}
          onChange={(e) => {
            setFilter(e.target.value);
          }}
        />
        <span className="muted small">
          {needle
            ? `${String(shown.length)} of ${String(log.entries.length)}`
            : `${String(log.entries.length)} lines`}
        </span>
      </div>
      <div className="transcript">
        {shown.map((entry, i) => (
          <Entry key={i} entry={entry} needle={needle} />
        ))}
        {shown.length === 0 && <div className="empty muted">Nothing matches.</div>}
        <div ref={bottom} />
      </div>
    </div>
  );
}

function Entry({ entry, needle }: { entry: TranscriptEntry; needle: string }) {
  const time = new Date(entry.ts).toLocaleTimeString([], { hour12: false });
  return (
    <div className={`le t-${entry.tag}`}>
      <span className="le-ts">{time}</span>
      <span className="le-tag">{entry.tag}</span>
      <div className="le-body">
        {entry.source && (
          <span className="le-src">
            <Marked text={entry.source} needle={needle} />{" "}
          </span>
        )}
        <Marked text={entry.body} needle={needle} />
      </div>
    </div>
  );
}

// Every occurrence of the search term, highlighted in place.
function Marked({ text, needle }: { text: string; needle: string }) {
  if (!needle) return <>{text}</>;
  const lower = text.toLowerCase();
  const parts: ReactNode[] = [];
  let at = 0;
  for (let hit = lower.indexOf(needle); hit !== -1; hit = lower.indexOf(needle, at)) {
    parts.push(text.slice(at, hit));
    parts.push(<mark key={hit}>{text.slice(hit, hit + needle.length)}</mark>);
    at = hit + needle.length;
  }
  parts.push(text.slice(at));
  return (
    <>
      {parts.map((p, i) => (
        <Fragment key={i}>{p}</Fragment>
      ))}
    </>
  );
}
