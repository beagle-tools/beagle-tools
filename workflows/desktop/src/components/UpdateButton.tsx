// Top-bar pill that appears only when a newer release is available.
// Checks on mount, every 30 minutes, and whenever the window regains
// focus. One click downloads the signed update, installs it and
// relaunches; a failure lands in the error bar and re-enables the pill.

import { useEffect, type ReactElement } from "react";

import { useUpdates } from "../state/updates";

const CHECK_EVERY_MS = 30 * 60 * 1000;

export function UpdateButton(): ReactElement | null {
  const available = useUpdates((s) => s.available);
  const installing = useUpdates((s) => s.installing);
  const check = useUpdates((s) => s.check);
  const install = useUpdates((s) => s.install);

  useEffect(() => {
    const run = (): void => {
      void check();
    };
    run();
    const timer = setInterval(run, CHECK_EVERY_MS);
    window.addEventListener("focus", run);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", run);
    };
  }, [check]);

  if (available === null) return null;

  return (
    <button
      type="button"
      className="update-pill"
      data-testid="update-pill"
      disabled={installing}
      title={`Restart into Agentdesk v${available}`}
      onClick={() => void install()}
    >
      {installing ? "Updating…" : `Update to v${available}`}
    </button>
  );
}
