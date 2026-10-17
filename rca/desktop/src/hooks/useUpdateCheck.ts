// Background update check: on mount, every 30 minutes, and whenever the
// window regains focus. Exposes the newer version (or null) and an install
// action that downloads, installs and relaunches.

import { useCallback, useEffect, useState } from "react";

import { checkForUpdate, installUpdateAndRelaunch } from "../api";

export const CHECK_EVERY_MS = 30 * 60 * 1000;

export interface UpdateCheck {
  /** The newer version available, or `null` when up to date / unknown. */
  available: string | null;
  installing: boolean;
  /** The last install failure, shown on the pill until the next try. */
  error: string | null;
  install: () => void;
}

export function useUpdateCheck(): UpdateCheck {
  const [available, setAvailable] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    const run = (): void => {
      void checkForUpdate().then((version) => {
        if (!cancelled && version !== null) {
          setAvailable(version);
        }
      });
    };
    run();
    const timer = setInterval(run, CHECK_EVERY_MS);
    window.addEventListener("focus", run);
    return () => {
      cancelled = true;
      clearInterval(timer);
      window.removeEventListener("focus", run);
    };
  }, []);

  const install = useCallback(() => {
    setInstalling(true);
    setError(null);
    installUpdateAndRelaunch().catch((cause: unknown) => {
      setInstalling(false);
      setError(cause instanceof Error ? cause.message : String(cause));
    });
  }, []);

  return { available, installing, error, install };
}
