// The version list: the installed version and every published rca release,
// with one-click install of any of them (upgrade or rollback — installs are
// signature-checked either way, then the app relaunches). Esc or a click
// on the backdrop closes it.

import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";

import { installAppVersion, listAppReleases } from "../api";
import { publishedDay, releaseAction, sortReleases } from "../lib/versions";
import type { AppRelease } from "../types";

const PAGE_SIZE = 5;

const describe = (cause: unknown): string =>
  cause instanceof Error ? cause.message : String(cause);

interface ReleaseRowProps {
  release: AppRelease;
  current: string;
  installing: string | null;
  onInstall: (release: AppRelease) => void;
}

function ReleaseRow({ release, current, installing, onInstall }: ReleaseRowProps): JSX.Element {
  const action = releaseAction(release, current);
  let control: JSX.Element;
  if (action === "current") {
    control = <span className="version-badge">current</span>;
  } else if (action === "unavailable") {
    control = <span className="version-note">not installable</span>;
  } else if (installing === release.tag) {
    control = <span className="version-note">installing…</span>;
  } else {
    control = (
      <button
        type="button"
        className={action === "update" ? "version-install primary" : "version-install"}
        disabled={installing !== null}
        title={`install ${release.tag} and relaunch`}
        onClick={() => {
          onInstall(release);
        }}
      >
        {action === "update" ? "update" : "roll back"}
      </button>
    );
  }
  return (
    <li className="version-row">
      <span className="version-tag">{release.tag}</span>
      <span className="version-date">{publishedDay(release)}</span>
      {control}
    </li>
  );
}

interface VersionsOverlayProps {
  current: string;
  onClose: () => void;
}

export function VersionsOverlay({ current, onClose }: VersionsOverlayProps): JSX.Element {
  const [releases, setReleases] = useState<AppRelease[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [installing, setInstalling] = useState<string | null>(null);
  const [visible, setVisible] = useState(PAGE_SIZE);

  // State is only set from the promise callbacks, so the effect below can
  // call this directly.
  const fetchReleases = useCallback(() => {
    listAppReleases()
      .then((list) => {
        setReleases(sortReleases(list));
      })
      .catch((cause: unknown) => {
        setError(describe(cause));
      });
  }, []);

  useEffect(() => {
    fetchReleases();
  }, [fetchReleases]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent): void => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
    };
  }, [onClose]);

  const refresh = (): void => {
    setError(null);
    setReleases(null);
    setVisible(PAGE_SIZE);
    fetchReleases();
  };

  const install = (release: AppRelease): void => {
    setInstalling(release.tag);
    installAppVersion(release.tag).catch((cause: unknown) => {
      setInstalling(null);
      setError(describe(cause));
    });
  };

  const hidden = (releases?.length ?? 0) - visible;
  return (
    <div className="overlay-backdrop" onClick={onClose} role="presentation">
      <div
        className="versions-sheet fade-in"
        role="dialog"
        aria-label="Versions"
        onClick={(event) => {
          event.stopPropagation();
        }}
      >
        <header className="versions-head">
          <h2>versions</h2>
          <button type="button" className="version-install" onClick={refresh}>
            refresh
          </button>
        </header>
        <p className="version-note">
          Beagle <strong>v{current || "…"}</strong> — install any release; the app restarts into it
        </p>
        {error !== null ? <div className="error-banner">{error}</div> : null}
        {releases === null && error === null ? <p className="version-note">loading…</p> : null}
        {releases?.length === 0 ? <p className="version-note">no releases published yet</p> : null}
        <ul className="version-list">
          {releases?.slice(0, visible).map((release) => (
            <ReleaseRow
              key={release.tag}
              release={release}
              current={current}
              installing={installing}
              onInstall={install}
            />
          ))}
        </ul>
        {hidden > 0 ? (
          <button
            type="button"
            className="sidebar-footnote"
            onClick={() => {
              setVisible((count) => count + PAGE_SIZE);
            }}
          >
            show {Math.min(PAGE_SIZE, hidden)} more ({hidden} older)
          </button>
        ) : null}
      </div>
    </div>
  );
}
