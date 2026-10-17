// The `?` overlay: renders the shortcut registry. Closes on Esc,
// backdrop click, or ✕ — it's ephemeral help, not a content pane.

import type { ReactElement } from "react";
import { Keyboard, X } from "lucide-react";

import { SHORTCUT_GROUPS } from "../lib/shortcuts";
import { useKeyNav } from "../state/keyNav";

export function ShortcutHelp(): ReactElement | null {
  const open = useKeyNav((s) => s.helpOpen);
  const setHelp = useKeyNav((s) => s.setHelp);
  if (!open) return null;

  return (
    <div
      className="animate-fade-in fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-6 backdrop-blur-sm"
      onMouseDown={() => {
        setHelp(false);
      }}
    >
      <div
        className="animate-fade-up max-h-full w-[min(90%,44rem)] overflow-y-auto rounded-xl border border-edge bg-panel p-4 shadow-2xl"
        onMouseDown={(e) => {
          e.stopPropagation();
        }}
      >
        <div className="mb-3 flex items-center gap-2">
          <Keyboard size={15} className="text-muted" />
          <span className="text-sm font-semibold text-cream">keyboard shortcuts</span>
          <button
            type="button"
            onClick={() => {
              setHelp(false);
            }}
            title="close (esc)"
            className="ml-auto inline-flex size-7 items-center justify-center rounded-lg text-muted transition-colors hover:bg-panel-2 hover:text-cream"
          >
            <X size={14} />
          </button>
        </div>
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
          {SHORTCUT_GROUPS.map((group) => (
            <section key={group.title}>
              <h3 className="mb-1.5 text-[11px] uppercase tracking-wide text-muted">
                {group.title}
              </h3>
              <div className="flex flex-col gap-1">
                {group.items.map((item) => (
                  <div key={item.keys} className="flex items-center gap-2 text-xs">
                    <kbd className="shrink-0 rounded border border-edge bg-panel-2 px-1.5 py-0.5 font-mono text-[10px] text-cream">
                      {item.keys}
                    </kbd>
                    <span className="text-muted">{item.label}</span>
                  </div>
                ))}
              </div>
            </section>
          ))}
        </div>
        <p className="mt-4 text-[10px] text-muted">
          single-letter keys are inactive while typing · actions that write to GitHub are never
          bound to keys
        </p>
      </div>
    </div>
  );
}
