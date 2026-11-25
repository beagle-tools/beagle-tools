// The repo layout (docs/LAYOUT.md) as a check: every tool is one top-level
// folder named like its `beagle` subcommand, with one folder per part you
// can run, and every crate sits where the layout says. Pure (no fs), so
// scripts/ci/layout.test.mjs can feed it any tree.

/** `beagle` subcommands that are not tools: they have no folder (rule 5). */
export const NOT_TOOLS = new Set(["serve"]);

/** Top-level folders besides the tools. */
export const REPO_DIRS = new Set([
  "beagle",
  "utils",
  "docs",
  "scripts",
  "standards",
  "packaging",
  "skills",
  // The one exception to "every app is a tool's part": the Beagle iPhone app
  // shows every tool, so it belongs to none (rule 9).
  "ios",
  ".github",
  ".claude",
]);

/**
 * What utils/ holds: the Rust crates (beagle-utils at the root of utils/,
 * beagle-tui in tui/) and the Swift package BeagleKit in swift/.
 */
export const UTILS_ENTRIES = new Set(["Cargo.toml", "src", "tui", "swift", "README.md", ".gitignore"]);

/** A tool's runnable parts (rule 2). */
export const PARTS = new Set(["tui", "daemon", "desktop", "web"]);
/** What else a tool's top level may hold (rule 6). */
export const TOOL_DIRS = new Set(["lib", "docs", "assets", "scripts", "deploy", "examples", "skills"]);
export const TOOL_FILES = new Set(["README.md", "AGENTS.md", "CLAUDE.md", "CHANGELOG.md", "Makefile", ".gitignore"]);

/** Tool names from beagle/src/tools.rs (`name: "…"` in TOOLS). */
export function toolNames(toolsRs) {
  const start = toolsRs.indexOf("pub const TOOLS");
  const body = start < 0 ? "" : toolsRs.slice(start, toolsRs.indexOf("];", start));
  return [...body.matchAll(/name:\s*"([^"]+)"/g)].map((m) => m[1]).filter((n) => !NOT_TOOLS.has(n));
}

/** Where a crate's Cargo.toml may live, given the tool names. */
function crateAllowed(path, tools) {
  const fixed = new Set([
    "Cargo.toml",
    "beagle/Cargo.toml",
    "beagle/serve/Cargo.toml",
    "utils/Cargo.toml",
    "utils/tui/Cargo.toml",
  ]);
  if (fixed.has(path)) return true;
  const parts = path.split("/");
  if (!tools.has(parts[0])) return false;
  const rest = parts.slice(1).join("/");
  return (
    rest === "tui/Cargo.toml" ||
    rest === "daemon/Cargo.toml" ||
    /^lib\/[^/]+\/Cargo\.toml$/.test(rest) ||
    // A Tauri shell: its manifest in desktop/ or desktop/src-tauri/ (open choice 6).
    rest === "desktop/Cargo.toml" ||
    rest === "desktop/src-tauri/Cargo.toml"
  );
}

/**
 * Checks the layout. `read(path)` returns a repo file's text, `files` lists
 * every repo file (repo-relative, build output already skipped). Returns
 * the violations.
 */
export function checkLayout(read, files) {
  const errors = [];
  const names = toolNames(read("beagle/src/tools.rs"));
  if (names.length === 0) errors.push("beagle/src/tools.rs: no tools found in TOOLS");
  const tools = new Set(names);

  // 1. Every tool is a top-level folder with a tui/ crate.
  for (const tool of tools) {
    if (!files.includes(`${tool}/tui/Cargo.toml`)) {
      errors.push(`${tool}/tui/Cargo.toml: missing — \`beagle ${tool}\` must live in ${tool}/tui/ (docs/LAYOUT.md)`);
    }
  }

  const seenTop = new Set();
  const seenToolEntry = new Set();
  for (const file of files) {
    const parts = file.split("/");
    if (parts.length === 1) continue; // root files
    const top = parts[0];

    // 2. Every top-level folder is a tool or a known repo folder.
    if (!tools.has(top) && !REPO_DIRS.has(top)) {
      if (!seenTop.has(top)) {
        seenTop.add(top);
        errors.push(
          `${top}/: not a \`beagle\` tool or a repo folder — a tool is a top-level folder named like its subcommand (docs/LAYOUT.md)`,
        );
      }
      continue;
    }

    // 3a. utils/ holds the shared Rust crates and the shared Swift package.
    if (top === "utils" && !UTILS_ENTRIES.has(parts[1]) && !seenToolEntry.has(`utils/${parts[1]}`)) {
      seenToolEntry.add(`utils/${parts[1]}`);
      errors.push(
        `utils/${parts[1]}: not allowed in utils/ — Rust goes in src/ (beagle-utils) or tui/ (beagle-tui), Swift in swift/ (BeagleKit) (docs/LAYOUT.md)`,
      );
    }

    // 3. A tool's top level holds only its parts and the allowed extras.
    if (tools.has(top)) {
      const entry = parts[1];
      const isDir = parts.length > 2;
      const ok = isDir ? PARTS.has(entry) || TOOL_DIRS.has(entry) : TOOL_FILES.has(entry);
      if (!ok && entry === "ios" && isDir && !seenToolEntry.has(`${top}/ios`)) {
        seenToolEntry.add(`${top}/ios`);
        errors.push(
          `${top}/ios/: not allowed — the one iPhone app is ios/ at the repo root; a tool's screens go in ios/Beagle/Tools/ (docs/LAYOUT.md)`,
        );
      } else if (!ok && !seenToolEntry.has(`${top}/${entry}`)) {
        seenToolEntry.add(`${top}/${entry}`);
        errors.push(
          `${top}/${entry}${isDir ? "/" : ""}: not allowed in a tool folder — use a part (${[...PARTS].map((p) => `${p}/`).join(", ")}), lib/<name>/, or ${[...TOOL_DIRS].filter((d) => d !== "lib").map((d) => `${d}/`).join(", ")} (docs/LAYOUT.md)`,
        );
      }
    }

    // 4. Every crate sits where the layout says.
    if (parts.at(-1) === "Cargo.toml" && !crateAllowed(file, tools)) {
      errors.push(
        `${file}: a crate here breaks the layout — use <tool>/tui, <tool>/daemon, <tool>/lib/<name>, <tool>/desktop or utils/ (docs/LAYOUT.md)`,
      );
    }
  }
  return errors;
}
