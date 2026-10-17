// Header pill that appears when a newer release is available. Checks
// on launch, every 30 minutes in the background, and whenever the
// window regains focus. One click downloads the signed update,
// installs it, and relaunches the app; failures land in the toast host
// and re-enable the button.

import { Download } from "lucide-react";
import { useEffect, useState } from "react";
import type { ReactElement } from "react";

import { checkForUpdate, installUpdateAndRelaunch } from "../lib/updater";
import { useToasts } from "../state/toasts";

const CHECK_EVERY_MS = 30 * 60 * 1000;

export function UpdateButton(): ReactElement | null {
  const push = useToasts((s) => s.push);
  const [version, setVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);

  useEffect(() => {
    const run = () => {
      void checkForUpdate().then((v) => {
        if (v !== null) setVersion(v);
      });
    };
    run();
    const timer = setInterval(run, CHECK_EVERY_MS);
    window.addEventListener("focus", run);
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", run);
    };
  }, []);

  if (version === null) return null;

  return (
    <button
      type="button"
      disabled={installing}
      title={`restart into Tandem v${version}`}
      onClick={() => {
        setInstalling(true);
        installUpdateAndRelaunch().catch((e: unknown) => {
          setInstalling(false);
          push("error", e instanceof Error ? e.message : String(e));
        });
      }}
      className="inline-flex items-center gap-1.5 whitespace-nowrap rounded-lg bg-sky-deep px-2.5 py-1 text-[11px] font-semibold text-white shadow-sm shadow-sky-deep/30 transition-all duration-150 hover:bg-sky hover:shadow-md active:scale-[0.97] disabled:pointer-events-none disabled:opacity-60"
    >
      {installing ? (
        <>
          <span className="size-3 animate-spin rounded-full border-2 border-white/30 border-t-white" />
          updating…
        </>
      ) : (
        <>
          <Download size={12} />
          {`update v${version}`}
        </>
      )}
    </button>
  );
}
