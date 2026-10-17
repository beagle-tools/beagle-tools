// Header pill that appears only when a newer release is available. One
// click downloads the signed update, installs it, and relaunches; a failure
// re-enables the pill and puts the reason in its tooltip.

import type { JSX } from "react";

import type { UpdateCheck } from "../hooks/useUpdateCheck";

export function UpdatePill({ update }: { update: UpdateCheck }): JSX.Element | null {
  const { available, installing, error, install } = update;
  if (available === null) {
    return null;
  }
  return (
    <button
      type="button"
      className={error === null ? "update-pill" : "update-pill failed"}
      disabled={installing}
      onClick={install}
      title={error ?? `restart into Beagle v${available}`}
      data-testid="update-pill"
    >
      {installing ? "updating…" : error === null ? `update v${available}` : "update failed — retry"}
    </button>
  );
}
