// Keyboard shortcut registry — the single source the help overlay (?)
// renders from. Dispatch lives in ShortcutManager; keep the two in sync.
// Design rule: nothing that writes to GitHub is ever bound to a key.

export interface Shortcut {
  keys: string;
  label: string;
}

export const SHORTCUT_GROUPS: { title: string; items: Shortcut[] }[] = [
  {
    title: "triage",
    items: [
      { keys: "j / k", label: "next / previous pull request" },
      { keys: "↵", label: "open highlighted pull request" },
      { keys: "1 2 3 4", label: "review status tabs: req · mine · mentions · find" },
      { keys: "/", label: "filter the list you're looking at" },
      { keys: "r", label: "refresh current list + PR" },
      { keys: "esc", label: "back to the list" },
    ],
  },
  {
    title: "views",
    items: [
      { keys: "⌘K", label: "jump to any pull request" },
      { keys: "⌘1 / ⌘2", label: "review status / agents dashboard" },
      { keys: "⌘,", label: "settings" },
      { keys: "⌘B", label: "toggle the right pane" },
      { keys: "⌘[ / ⌘]", label: "back / forward through the last 5" },
      { keys: "⌘F", label: "find text on this screen" },
      { keys: "⌘+ ⌘− ⌘0", label: "zoom in / out / reset" },
      { keys: "?", label: "this help" },
    ],
  },
  {
    title: "inside a PR",
    items: [
      { keys: "] / [", label: "next / previous file in the diff" },
      { keys: "n / p", label: "next / previous comment thread" },
      { keys: "x", label: "collapse / expand current file" },
      { keys: "⌘↵", label: "submit the composer you're typing in" },
    ],
  },
  {
    title: "agents (local only — GitHub writes are never on a key)",
    items: [
      { keys: "a", label: "run the selected review agent on this PR" },
      { keys: "⇧A", label: "agents dashboard" },
    ],
  },
];
