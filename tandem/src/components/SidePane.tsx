// Collapsible side pane. Expanded: just the pane, with a round handle
// on its inner edge to collapse. Collapsed: a slim rail with status
// icons that never fully disappears; hovering it slides the pane out as
// an overlay, and the overlay's pin handle freezes it open again.

import { ChevronsLeft, ChevronsRight, Pin } from "lucide-react";
import type { ReactElement, ReactNode } from "react";
import { useState } from "react";

import { useModalHold } from "./ui";

interface SidePaneProps {
  side: "left" | "right";
  /** true = expanded (pane visible), false = collapsed to the rail. */
  pinned: boolean;
  onTogglePin: () => void;
  /** Icons shown on the collapsed rail (status indicators, counts). */
  rail: ReactNode;
  /** Tailwind width class for the pane, e.g. "w-72". */
  widthClass: string;
  children: ReactNode;
}

export function SidePane(props: SidePaneProps): ReactElement {
  const [hovering, setHovering] = useState(false);
  // A modal rendered from this pane's content must keep the overlay
  // alive even when the mouse wanders off (e.g. to another monitor).
  const modalOpen = useModalHold((s) => s.count > 0);
  const isLeft = props.side === "left";
  const borderClass = isLeft ? "border-r border-edge" : "border-l border-edge";
  const CollapseIcon = isLeft ? ChevronsLeft : ChevronsRight;
  const ExpandIcon = isLeft ? ChevronsRight : ChevronsLeft;

  const handleClass = `absolute top-1/2 z-40 -translate-y-1/2 ${
    isLeft ? "-right-3" : "-left-3"
  } flex size-6 items-center justify-center rounded-full border border-edge bg-panel text-muted shadow-md transition-colors hover:border-sky hover:text-cream`;

  const pane = (
    <div className={`h-full overflow-hidden bg-panel ${props.widthClass} ${borderClass}`}>
      {props.children}
    </div>
  );

  if (props.pinned) {
    return (
      <div className="relative h-full shrink-0">
        {pane}
        <button
          type="button"
          onClick={props.onTogglePin}
          title="collapse pane"
          className={handleClass}
        >
          <CollapseIcon size={13} />
        </button>
      </div>
    );
  }

  return (
    <div
      className="relative h-full shrink-0"
      onMouseLeave={() => {
        setHovering(false);
      }}
    >
      <div
        onMouseEnter={() => {
          setHovering(true);
        }}
        className={`flex h-full w-9 flex-col items-center gap-3 bg-panel py-3 ${borderClass}`}
      >
        <div className="flex flex-col items-center gap-3 text-muted">{props.rail}</div>
        <button
          type="button"
          onClick={props.onTogglePin}
          title="expand pane"
          className="mt-auto inline-flex size-7 items-center justify-center rounded-md text-muted transition-colors hover:bg-panel-2 hover:text-cream"
        >
          <ExpandIcon size={14} />
        </button>
      </div>

      {hovering || modalOpen ? (
        <div
          className={`animate-fade-in absolute top-0 z-30 h-full ${
            isLeft ? "left-full" : "right-full"
          }`}
        >
          <div className="relative h-full shadow-2xl">
            {pane}
            <button
              type="button"
              onClick={() => {
                props.onTogglePin();
                setHovering(false);
              }}
              title="pin pane open"
              className={handleClass}
            >
              <Pin size={12} />
            </button>
          </div>
        </div>
      ) : null}
    </div>
  );
}
