#!/usr/bin/env node
// Stamp a release version into every place the unified `beagle` command
// (release app `cli`) declares one:
//
//   CRATES/Cargo.toml   each crate's [package] version
//   ../Cargo.lock       those crates' entries in the repo-root lockfile,
//                       so `cargo build --locked` passes
//
// Path dependencies from other apps (rca's `beagle`, `tandem-cli`,
// `beagle-utils`) keep their own versions: they are not ours to stamp.
//
// Release CI runs it from beagle/ (`node scripts/set-version.mjs 1.2.3`)
// before building; the result is never committed. Edits are textual so the
// files keep their formatting, and every edit is checked to have landed —
// a location this script silently missed would ship a binary whose
// `--version` disagrees with its tag.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^\d+\.\d+\.\d+$/;

/** Replace `version = "..."` inside one TOML table; `hit` says if it was there. */
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

/**
 * The crates that release as `beagle-cli`, relative to beagle/: the command
 * itself, `beagle serve`, and the tools that live only in `beagle`
 * (`<tool>/tui`).
 */
export const CRATES = [
  ".",
  "serve",
  "../recap/tui",
  "../scout/tui",
  "../server-mode/tui",
  "../standup/tui",
];

/** Manifests of the CRATES, under `appDir` (beagle/). */
export function memberManifests(appDir) {
  return CRATES.map((dir) => join(appDir, dir, "Cargo.toml"));
}

/** `[package] name` of a manifest. */
export function packageName(text) {
  return /^\[package\][^[]*?^name\s*=\s*"([^"]+)"/ms.exec(text)?.[1] ?? null;
}

/**
 * Set the version of the named path packages in a Cargo.lock. Registry and
 * git packages carry a `source =` line and are never touched.
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

/**
 * Stamp `version` for the CRATES, from `appDir` (the beagle/ directory). The crates
 * are members of the repo-root workspace, whose lockfile is `../Cargo.lock`.
 */
export function setVersion(appDir, version) {
  if (!SEMVER.test(version)) throw new Error(`not a X.Y.Z version: ${version}`);

  // Compute every write first, so a failure leaves nothing half-stamped.
  const writes = new Map();
  const names = new Set();
  for (const path of memberManifests(appDir)) {
    const text = readFileSync(path, "utf8");
    const name = packageName(text);
    if (!name) throw new Error(`${path}: no [package] name`);
    names.add(name);
    const own = stampTomlTable(text, "package", version);
    if (!own.hit) throw new Error(`${path}: no [package] version`);
    writes.set(path, own.text);
  }
  if (names.size === 0) throw new Error(`${appDir}: no crates`);

  const lock = join(appDir, "..", "Cargo.lock");
  const res = stampLock(readFileSync(lock, "utf8"), names, version);
  const missing = [...names].filter((n) => !res.stamped.has(n));
  if (missing.length) throw new Error(`${lock}: no entry for ${missing.join(", ")}`);
  writes.set(lock, res.text);

  for (const [path, text] of writes) writeFileSync(path, text);
  return [...writes.keys()];
}

const invokedDirectly =
  process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const version = process.argv[2] ?? "";
  if (!SEMVER.test(version)) {
    console.error(`usage: node scripts/set-version.mjs <X.Y.Z> (got "${version}")`);
    process.exit(1);
  }
  const appDir = resolve(dirname(fileURLToPath(import.meta.url)), "..");
  for (const path of setVersion(appDir, version)) console.log(`${path} -> ${version}`);
}
