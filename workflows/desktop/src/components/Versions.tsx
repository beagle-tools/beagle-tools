// The installed version and every published Agentdesk release, with a
// one-click switch to any of them — upgrade or roll back. Installs are
// signature-checked either way, then the app relaunches into the choice.
// Opened from the top bar's version button, which also shows what runs.

import { useEffect, useState, type ReactElement } from "react";

import type { AppRelease } from "../lib/types";
import { releaseAction } from "../lib/versions";
import { useUpdates } from "../state/updates";

const PAGE_SIZE = 8;

function ReleaseRow({ release, current }: { release: AppRelease; current: string }): ReactElement {
  const switching = useUpdates((s) => s.switching);
  const installVersion = useUpdates((s) => s.installVersion);
  const action = releaseAction(release, current);

  return (
    <li className="release">
      <span className="mono">{release.tag}</span>
      {action === "current" && <span className="pill">current</span>}
      {release.published_at && (
        <span className="muted small">{release.published_at.slice(0, 10)}</span>
      )}
      <span className="release-action">
        {action === "unavailable" && <span className="muted small">not installable</span>}
        {(action === "update" || action === "rollback") &&
          (switching === release.tag ? (
            <span className="muted small">Installing…</span>
          ) : (
            <button
              className={action === "update" ? "primary" : ""}
              disabled={switching !== null}
              title={`Install ${release.tag} and relaunch`}
              onClick={() => void installVersion(release.tag)}
            >
              {action === "update" ? "Update" : "Roll back"}
            </button>
          ))}
      </span>
    </li>
  );
}

function VersionsDialog({ onClose }: { onClose: () => void }): ReactElement {
  const current = useUpdates((s) => s.current);
  const releases = useUpdates((s) => s.releases);
  const releasesError = useUpdates((s) => s.releasesError);
  const loadReleases = useUpdates((s) => s.loadReleases);
  const [visible, setVisible] = useState(PAGE_SIZE);

  useEffect(() => {
    void loadReleases();
  }, [loadReleases]);

  const hidden = releases ? releases.length - visible : 0;

  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div
        className="modal"
        role="dialog"
        aria-label="Versions"
        onClick={(e) => {
          e.stopPropagation();
        }}
      >
        <h2>Versions</h2>
        <p className="muted small">
          Agentdesk <span className="mono">v{current || "…"}</span> — switch to any release below;
          the app restarts into it.
        </p>
        {releases === null && releasesError === null && (
          <p className="muted small">Loading releases…</p>
        )}
        {releasesError !== null && <p className="release-error">{releasesError}</p>}
        {releases?.length === 0 && <p className="muted small">No releases published yet.</p>}
        <ul className="releases">
          {releases?.slice(0, visible).map((r) => (
            <ReleaseRow key={r.tag} release={r} current={current} />
          ))}
        </ul>
        <footer>
          {hidden > 0 && (
            <button
              onClick={() => {
                setVisible((v) => v + PAGE_SIZE);
              }}
            >
              Show {Math.min(PAGE_SIZE, hidden)} more ({hidden} older)
            </button>
          )}
          <button onClick={() => void loadReleases()}>Refresh</button>
          <button onClick={onClose}>Close</button>
        </footer>
      </div>
    </div>
  );
}

/** Top-bar button showing the running version; opens the Versions dialog. */
export function VersionButton(): ReactElement {
  const current = useUpdates((s) => s.current);
  const loadCurrent = useUpdates((s) => s.loadCurrent);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    void loadCurrent();
  }, [loadCurrent]);

  return (
    <>
      <button
        className="version"
        title="Installed version — switch or roll back"
        onClick={() => {
          setOpen(true);
        }}
      >
        {current ? `v${current}` : "Versions"}
      </button>
      {open && (
        <VersionsDialog
          onClose={() => {
            setOpen(false);
          }}
        />
      )}
    </>
  );
}
