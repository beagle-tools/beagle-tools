// ⌘F / the navbar magnifier: a floating pane holding the same find-PR
// browser the review-status tab uses — pick any repo across your orgs,
// or paste a PR URL, from wherever you happen to be.

import { useEffect } from "react";
import type { ReactElement } from "react";
import { create } from "zustand";

import { useAppStore } from "../state/store";
import { FindPrTab } from "./FindPrTab";
import { Modal } from "./ui";

interface FindStore {
  open: boolean;
  setOpen: (open: boolean) => void;
}

export const usePrSearch = create<FindStore>((set) => ({
  open: false,
  setOpen: (open) => {
    set({ open });
  },
}));

export function PrSearchPane(): ReactElement | null {
  const open = usePrSearch((s) => s.open);
  const setOpen = usePrSearch((s) => s.setOpen);
  const selectedPr = useAppStore((s) => s.selectedPr);

  // Opening a PR is the point of the pane — once one is open, get out
  // of the way rather than making you close it yourself.
  useEffect(() => {
    if (open) setOpen(false);
    // Only when the opened PR changes, never on open itself.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedPr]);

  if (!open) return null;
  return (
    <Modal
      title="find a pull request"
      onClose={() => {
        setOpen(false);
      }}
    >
      <FindPrTab />
    </Modal>
  );
}
