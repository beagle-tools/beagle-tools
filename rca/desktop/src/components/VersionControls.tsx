// The updater corner of the header: the update pill (only when a newer
// release exists) and the version button that opens the version list.

import { useCallback, useEffect, useState } from "react";
import type { JSX } from "react";

import { appVersion } from "../api";
import { useUpdateCheck } from "../hooks/useUpdateCheck";
import { UpdatePill } from "./UpdatePill";
import { VersionsOverlay } from "./VersionsOverlay";

export function VersionControls(): JSX.Element {
  const update = useUpdateCheck();
  const [current, setCurrent] = useState("");
  const [open, setOpen] = useState(false);

  useEffect(() => {
    appVersion()
      .then(setCurrent)
      .catch((cause: unknown) => {
        console.warn("could not read the app version", cause);
      });
  }, []);

  const close = useCallback(() => {
    setOpen(false);
  }, []);

  return (
    <div className="version-controls">
      <UpdatePill update={update} />
      <button
        type="button"
        className="version-button"
        title="versions — update or roll back"
        onClick={() => {
          setOpen(true);
        }}
      >
        {current === "" ? "versions" : `v${current}`}
      </button>
      {open ? <VersionsOverlay current={current} onClose={close} /> : null}
    </div>
  );
}
