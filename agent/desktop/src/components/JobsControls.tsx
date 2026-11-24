import { useEffect, type ReactElement, type RefObject } from "react";

export type JobsKey = "search" | "running" | "all" | "hidden";

// `/` focuses the search; `f` / `a` switch Running / All (the same keys as
// `beagle agent top`), `H` toggles the hidden projects. Anything else,
// `r` included (agenttop's reload), does nothing here.
export function jobsKeyAction(key: string): JobsKey | null {
  switch (key) {
    case "/":
      return "search";
    case "f":
      return "running";
    case "a":
      return "all";
    case "H":
      return "hidden";
    default:
      return null;
  }
}

// Never while typing: an `f` in the search box is an `f`.
export function useJobsKeys(
  input: RefObject<HTMLInputElement | null>,
  setOnlyRunning: (v: boolean) => void,
  toggleHidden: () => void,
): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent): void => {
      if (e.metaKey || e.ctrlKey || e.altKey) return;
      const t = e.target;
      if (t instanceof HTMLElement && (t.tagName === "INPUT" || t.tagName === "TEXTAREA")) return;
      switch (jobsKeyAction(e.key)) {
        case "search":
          e.preventDefault();
          input.current?.focus();
          break;
        case "running":
          setOnlyRunning(true);
          break;
        case "all":
          setOnlyRunning(false);
          break;
        case "hidden":
          toggleHidden();
          break;
        case null:
          break;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [input, setOnlyRunning, toggleHidden]);
}

// The All | Running toggle: every job, or only those with a run live.
export function RunningFilter({
  onlyRunning,
  runningCount,
  setOnlyRunning,
}: {
  onlyRunning: boolean;
  runningCount: number;
  setOnlyRunning: (v: boolean) => void;
}): ReactElement {
  return (
    <div className="segmented" role="group" aria-label="Filter jobs">
      <button
        className={onlyRunning ? "" : "on"}
        aria-pressed={!onlyRunning}
        title="Every job (a)"
        onClick={() => {
          setOnlyRunning(false);
        }}
      >
        All
      </button>
      <button
        className={onlyRunning ? "on" : ""}
        aria-pressed={onlyRunning}
        title="Only jobs with a run live (f)"
        onClick={() => {
          setOnlyRunning(true);
        }}
      >
        Running{runningCount > 0 ? ` · ${String(runningCount)}` : ""}
      </button>
    </div>
  );
}

// ui.yaml's hidden projects (demo): off the page until this is ticked.
export function ShowHidden({
  count,
  checked,
  onChange,
}: {
  count: number;
  checked: boolean;
  onChange: (v: boolean) => void;
}): ReactElement | null {
  if (count === 0) return null;
  return (
    <label className="muted small showhidden" title="Projects hidden in ui.yaml (H)">
      <input
        type="checkbox"
        checked={checked}
        onChange={(e) => {
          onChange(e.target.checked);
        }}
      />
      Show hidden · {count}
    </label>
  );
}
