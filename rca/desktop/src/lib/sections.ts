// The nine markdown sections a workspace can contain, in tab order.
// Mirrors `SectionKind` in the CLI crate — file names are the on-disk
// format, which is the public API shared by every beagle frontend.

export interface Section {
  /** File name inside the workspace directory. */
  readonly file: string;
  /** Human-readable tab title. */
  readonly title: string;
}

export const SECTIONS: readonly Section[] = [
  { file: "summary.md", title: "Summary" },
  { file: "timeline.md", title: "Timeline" },
  { file: "root-cause.md", title: "Root Cause" },
  { file: "impact.md", title: "Impact" },
  { file: "remediation.md", title: "Fix" },
  { file: "commands.md", title: "Commands" },
  { file: "final-review.md", title: "Final Review" },
  { file: "notes.md", title: "Notes" },
  { file: "log.md", title: "Log" },
] as const;

/** The Diagrams pseudo-tab: not a markdown section, rendered unwrapped. */
export const DIAGRAMS_TAB: Section = { file: "__diagrams__", title: "Diagrams" };

/** The opt-in Commands section: its tab shows only when the file exists. */
export const COMMANDS_FILE = "commands.md";

/**
 * Every tab in TUI order: the narrative sections (Commands among them),
 * Diagrams, then Notes and Log.
 */
export const TABS: readonly Section[] = [
  ...SECTIONS.slice(0, 7),
  DIAGRAMS_TAB,
  ...SECTIONS.slice(7),
];

/**
 * The tabs a workspace shows, like the TUI: Commands only when the
 * workspace has `commands.md` (it is opt-in, never scaffolded).
 */
export function tabsFor(hasCommands: boolean): readonly Section[] {
  return hasCommands ? TABS : TABS.filter((tab) => tab.file !== COMMANDS_FILE);
}

/** The tab after/before `file` in `tabs`, wrapping at both ends. */
export function cycleTab(
  file: string,
  direction: 1 | -1,
  tabs: readonly Section[] = tabsFor(false),
): string {
  const index = tabs.findIndex((tab) => tab.file === file);
  const from = index === -1 ? 0 : index;
  const next = (from + direction + tabs.length) % tabs.length;
  return tabs[next]?.file ?? file;
}
