#!/usr/bin/env node
// Stamp a release version into every place workflows/ declares one:
//
//   Cargo.toml                    [workspace.package] version (agentd, agentctl,
//                                 agenttop, agent-core)
//   desktop/Cargo.toml            [workspace.package] version (agentdesk)
//   desktop/src-tauri/tauri.conf.json, desktop/package.json   "version"
//   Cargo.lock, desktop/Cargo.lock  the entries for the packages above, so
//                                 `cargo build --locked` still passes
//
// Release CI runs it from workflows/ (`node scripts/set-version.mjs 1.2.3`)
// before building; the result is never committed. Edits are textual so the
// files keep their formatting, and every edit is checked to have landed —
// a version location this script silently missed would ship a binary whose
// `--version` disagrees with its tag.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^\d+\.\d+\.\d+$/;

/** The two cargo workspaces (relative to workflows/). */
const WORKSPACES = [".", "desktop"];
const JSON_FILES = ["desktop/package.json", "desktop/src-tauri/tauri.conf.json"];

function edit(path, transform) {
  const before = readFileSync(path, "utf8");
  const after = transform(before);
  if (after !== before) writeFileSync(path, after);
}

/** Replace `version = "..."` inside one TOML table, failing if it is absent. */
export function stampTomlTable(text, table, version) {
  const lines = text.split("\n");
  let inTable = false;
  let hit = false;
  for (let i = 0; i < lines.length; i++) {
    const header = /^\s*\[([^\]]+)\]\s*$/.exec(lines[i]);
    if (header) {
      inTable = header[1].trim() === table;
      continue;
    }
    if (inTable && /^\s*version\s*=\s*"[^"]*"/.test(lines[i])) {
      lines[i] = lines[i].replace(/"[^"]*"/, `"${version}"`);
      hit = true;
    }
  }
  return { text: lines.join("\n"), hit };
}

/** Workspace member dirs from a `members = [...]` array. */
export function workspaceMembers(text) {
  const block = /^members\s*=\s*\[([^\]]*)\]/m.exec(text);
  if (!block) return [];
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

/** `[package] name` of a member manifest. */
function packageName(text) {
  const pkg = /^\[package\][^[]*?^name\s*=\s*"([^"]+)"/ms.exec(text);
  return pkg ? pkg[1] : null;
}

/**
 * Set the version of the named path packages in a Cargo.lock. Registry and
 * git packages carry a `source =` line and are never touched, so a crates.io
 * dependency that happens to share a name is safe.
 */
export function stampLock(text, names, version) {
  const stamped = new Set();
  const out = text
    .split(/(?=^\[\[package\]\]$)/m)
    .map((block) => {
      const name = /^name = "([^"]+)"$/m.exec(block)?.[1];
      if (!name || !names.has(name) || /^source = /m.test(block)) return block;
      stamped.add(name);
      return block.replace(/^version = "[^"]*"$/m, `version = "${version}"`);
    })
    .join("");
  return { text: out, stamped };
}

/** Set a top-level `"version"` in a JSON file without reformatting it. */
export function stampJson(text, version) {
  const out = text.replace(/^( {2}"version":\s*")[^"]*(")/m, `$1${version}$2`);
  if (JSON.parse(out).version !== version) throw new Error("no top-level version");
  return out;
}

/** Stamp `version` everywhere under `appDir` (the workflows/ directory). */
export function setVersion(appDir, version) {
  if (!SEMVER.test(version)) throw new Error(`not a X.Y.Z version: ${version}`);
  const touched = [];

  // Our own package names, from both workspaces' members: these (and only
  // these) get stamped in each lockfile.
  const names = new Set();
  for (const ws of WORKSPACES) {
    const manifest = join(appDir, ws, "Cargo.toml");
    const text = readFileSync(manifest, "utf8");
    const res = stampTomlTable(text, "workspace.package", version);
    if (!res.hit) throw new Error(`${manifest}: no [workspace.package] version`);
    writeFileSync(manifest, res.text);
    touched.push(manifest);
    for (const member of workspaceMembers(text)) {
      const path = join(appDir, ws, member, "Cargo.toml");
      const memberText = readFileSync(path, "utf8");
      const name = packageName(memberText);
      if (name) names.add(name);
      // A member pinning its own literal version (instead of
      // `version.workspace = true`) gets stamped too.
      const own = stampTomlTable(memberText, "package", version);
      if (own.hit) {
        writeFileSync(path, own.text);
        touched.push(path);
      }
    }
  }

  for (const ws of WORKSPACES) {
    const lock = join(appDir, ws, "Cargo.lock");
    let stamped = new Set();
    edit(lock, (text) => {
      const res = stampLock(text, names, version);
      stamped = res.stamped;
      return res.text;
    });
    if (stamped.size === 0) throw new Error(`${lock}: none of ${[...names].join(", ")} found`);
    touched.push(lock);
  }

  for (const rel of JSON_FILES) {
    const path = join(appDir, rel);
    edit(path, (text) => stampJson(text, version));
    touched.push(path);
  }
  return touched;
}

const invokedDirectly = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const version = process.argv[2] ?? "";
  if (!SEMVER.test(version)) {
    console.error(`usage: node scripts/set-version.mjs <X.Y.Z> (got "${version}")`);
    process.exit(1);
  }
  const appDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  for (const path of setVersion(appDir, version)) console.log(`${path} -> ${version}`);
}
