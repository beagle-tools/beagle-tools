// The local crates a crate links, read straight from Cargo.toml files (no
// cargo needed, so it runs in the node-only standards job).
//
// Follows `[dependencies]` / `[build-dependencies]` entries that are path
// dependencies, directly (`path = "…"`) or through the workspace
// (`x.workspace = true` → the root's `[workspace.dependencies]`). Dev
// dependencies are skipped: they are never linked into a release binary.

import { existsSync, readFileSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";

/** `[section]` → its `key = value` lines, for the sections we read. */
function sections(text) {
  const out = new Map();
  let current = null;
  for (const raw of text.split("\n")) {
    const line = raw.replace(/#.*$/, "").trim();
    const header = line.match(/^\[([^\]]+)\]$/);
    if (header) {
      current = header[1];
      if (!out.has(current)) out.set(current, []);
    } else if (current && line) {
      out.get(current).push(line);
    }
  }
  return out;
}

/** `name` → `{ path?, workspace? }` for one dependency table's lines. */
function deps(lines) {
  const out = new Map();
  for (const line of lines) {
    const m = line.match(/^([A-Za-z0-9_-]+)(\.workspace)?\s*=\s*(.*)$/);
    if (!m) continue;
    const [, name, dotted, value] = m;
    const path = value.match(/path\s*=\s*"([^"]+)"/)?.[1];
    const workspace = Boolean(dotted) || /workspace\s*=\s*true/.test(value);
    out.set(name, { path, workspace });
  }
  return out;
}

/** The nearest ancestor of `dir` whose Cargo.toml has a `[workspace]`. */
function workspaceRoot(dir) {
  for (let d = dir; ; d = dirname(d)) {
    const manifest = join(d, "Cargo.toml");
    if (existsSync(manifest) && /^\[workspace\]/m.test(readFileSync(manifest, "utf8"))) return d;
    if (dirname(d) === d) return null;
  }
}

/** Repo-relative dirs of every local crate `crateDir` links, itself included. */
export function linkedCrateDirs(repoRoot, crateDir) {
  const seen = new Set();
  const visit = (dir) => {
    const abs = resolve(repoRoot, dir);
    const rel = relative(repoRoot, abs);
    if (seen.has(rel)) return;
    seen.add(rel);
    const text = readFileSync(join(abs, "Cargo.toml"), "utf8");
    const own = sections(text);
    const ws = workspaceRoot(abs);
    const wsDeps = ws
      ? deps(sections(readFileSync(join(ws, "Cargo.toml"), "utf8")).get("workspace.dependencies") ?? [])
      : new Map();
    for (const table of ["dependencies", "build-dependencies"]) {
      for (const [name, dep] of deps(own.get(table) ?? [])) {
        if (dep.path) visit(join(abs, dep.path));
        else if (dep.workspace && wsDeps.get(name)?.path) visit(join(ws, wsDeps.get(name).path));
      }
    }
  };
  visit(crateDir);
  return [...seen].sort();
}
