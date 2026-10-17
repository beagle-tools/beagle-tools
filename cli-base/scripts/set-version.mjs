#!/usr/bin/env node
// Stamp a release version into every place cli-base/ declares one:
//
//   Cargo.toml   [workspace.package] version (beagle-cli)
//   Cargo.lock   the beagle-cli entry, so `cargo build --locked` still passes
//
// Release CI runs it from cli-base/ (`node scripts/set-version.mjs 1.2.3`)
// before building; the result is never committed. Edits are textual so the
// files keep their formatting, and each edit is checked to have landed — a
// missed location would ship a binary whose `--version` disagrees with its
// tag.

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const SEMVER = /^\d+\.\d+\.\d+$/;
/** Our own packages: only these are stamped in the lockfile. */
const PACKAGES = new Set(["beagle-cli"]);

/** Replace `version = "..."` inside one TOML table; `hit` says if it did. */
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

/** Stamp `version` into `appDir` (the cli-base/ directory). */
export function setVersion(appDir, version) {
  if (!SEMVER.test(version)) throw new Error(`not a X.Y.Z version: ${version}`);
  const manifest = join(appDir, "Cargo.toml");
  const toml = stampTomlTable(readFileSync(manifest, "utf8"), "workspace.package", version);
  if (!toml.hit) throw new Error(`${manifest}: no [workspace.package] version`);
  const lockPath = join(appDir, "Cargo.lock");
  const lock = stampLock(readFileSync(lockPath, "utf8"), PACKAGES, version);
  const missing = [...PACKAGES].filter((p) => !lock.stamped.has(p));
  if (missing.length) throw new Error(`${lockPath}: ${missing.join(", ")} not found`);
  writeFileSync(manifest, toml.text);
  writeFileSync(lockPath, lock.text);
  return [manifest, lockPath];
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
