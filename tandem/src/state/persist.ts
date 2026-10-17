// localStorage-backed UI preferences (theme, pane pins).

import type { Theme } from "./storeTypes";

const THEME_KEY = "tandem-theme";

export function loadTheme(): Theme {
  // Light is the default; dark only when explicitly chosen.
  return localStorage.getItem(THEME_KEY) === "dark" ? "dark" : "light";
}

export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme;
  localStorage.setItem(THEME_KEY, theme);
}

export function loadPinned(key: string): boolean {
  return localStorage.getItem(key) !== "false";
}

const VIEWER_KEY = "tandem-viewer";

export function loadViewer(): string | null {
  return localStorage.getItem(VIEWER_KEY);
}

export function saveViewer(login: string): void {
  localStorage.setItem(VIEWER_KEY, login);
}

const ZOOM_KEY = "tandem-zoom";

export function loadZoom(): number {
  const saved = Number(localStorage.getItem(ZOOM_KEY));
  return Number.isFinite(saved) && saved >= 70 && saved <= 160 ? saved : 100;
}

export function applyZoom(zoom: number): void {
  localStorage.setItem(ZOOM_KEY, String(zoom));
  // WebKit honors CSS zoom; scales the whole UI like a browser zoom.
  (document.body.style as CSSStyleDeclaration & { zoom: string }).zoom = `${String(zoom)}%`;
}

const AGENT_KEY = "tandem-last-agent";

/** Last agent the user picked in the agent panel — the `a` shortcut and
 * the panel's dropdown share it. */
export function loadAgent(): string {
  return localStorage.getItem(AGENT_KEY) ?? "";
}

export function saveAgent(name: string): void {
  localStorage.setItem(AGENT_KEY, name);
}
