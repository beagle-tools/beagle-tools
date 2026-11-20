// The section tab bar for the selected incident.

import type { JSX } from "react";

import type { Section } from "../lib/sections";

interface TabBarProps {
  /** This workspace's tabs (`tabsFor`). */
  tabs: readonly Section[];
  activeFile: string;
  onSelect: (file: string) => void;
}

export function TabBar({ tabs, activeFile, onSelect }: TabBarProps): JSX.Element {
  return (
    <div className="tab-bar" role="tablist">
      {tabs.map((section, index) => (
        <button
          key={section.file}
          type="button"
          role="tab"
          aria-selected={section.file === activeFile}
          className={section.file === activeFile ? "tab active" : "tab"}
          onClick={() => {
            onSelect(section.file);
          }}
        >
          {index + 1} {section.title}
        </button>
      ))}
    </div>
  );
}
