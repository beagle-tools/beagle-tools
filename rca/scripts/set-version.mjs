// Stamp one release version into every place the rca app records one — the
// CLI, its crates, the desktop app and the web app all ship as one version:
//   cli/Cargo.toml, crates/*/Cargo.toml, desktop/src-tauri/Cargo.toml,
//   desktop/src-tauri/tauri.conf.json, desktop/package.json, web/package.json,
//   and those crates' entries in Cargo.lock + desktop/src-tauri/Cargo.lock
//   (so `cargo build --locked` keeps working).
//
// Run from rca/ by release CI only; the result is never committed.
//   node scripts/set-version.mjs 0.30.0
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`usage: node scripts/set-version.mjs <major.minor.patch> (got ${version ?? ""})`);
  process.exit(1);
}

const JSON_FILES = ["desktop/package.json", "desktop/src-tauri/tauri.conf.json", "web/package.json"];
const crateDirs = readdirSync("crates", { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && existsSync(join("crates", entry.name, "Cargo.toml")))
  .map((entry) => join("crates", entry.name))
  .sort();
const MANIFESTS = ["cli", ...crateDirs, "desktop/src-tauri"].map((dir) => join(dir, "Cargo.toml"));
const LOCKS = ["Cargo.lock", "desktop/src-tauri/Cargo.lock"];

/** Replace exactly one match of `pattern` in `text`, or fail loudly. */
function replaceOne(path, text, pattern, replacement) {
  const matches = text.match(new RegExp(pattern.source, pattern.flags + "g"));
  if (matches?.length !== 1) {
    throw new Error(`${path}: expected one version to stamp, found ${matches?.length ?? 0}`);
  }
  return text.replace(pattern, replacement);
}

/** The `[package]` body of a manifest: from its header to the next header. */
function packageSection(text) {
  const start = text.indexOf("[package]\n");
  if (start < 0) return null;
  const rest = text.slice(start + "[package]\n".length);
  const end = rest.search(/^\[/m);
  return end < 0 ? rest : rest.slice(0, end);
}

// Validate and compute everything first so a failure leaves no file half-stamped.
const writes = new Map();

// JSON: rewrite the top-level "version" in place so formatting survives.
for (const path of JSON_FILES) {
  const text = replaceOne(path, readFileSync(path, "utf8"), /^ {2}"version": "[^"]*"/m, `  "version": "${version}"`);
  if (JSON.parse(text).version !== version) throw new Error(`${path}: top-level version was not stamped`);
  writes.set(path, text);
}

// Cargo.toml: each crate's [package] version.
const crateNames = [];
for (const path of MANIFESTS) {
  const manifest = readFileSync(path, "utf8");
  const pkg = packageSection(manifest);
  const name = pkg && /^name = "([^"]+)"$/m.exec(pkg)?.[1];
  if (!name) throw new Error(`${path}: no [package] name`);
  const stamped = replaceOne(path, pkg, /^version = "[^"]*"$/m, `version = "${version}"`);
  writes.set(path, manifest.replace(pkg, stamped));
  crateNames.push(name);
}

// Cargo.lock files: every rca crate that lockfile records (the desktop lock
// only knows `beagle` and `beagle-desktop`). Shared utils are not rca's.
for (const path of LOCKS) {
  let lock = readFileSync(path, "utf8");
  let found = 0;
  for (const name of crateNames) {
    const escaped = name.replace(/[-.]/g, "\\$&");
    const pattern = new RegExp(`^(\\[\\[package\\]\\]\\nname = "${escaped}"\\nversion = )"[^"]*"$`, "m");
    if (!pattern.test(lock)) continue;
    lock = replaceOne(path, lock, pattern, `$1"${version}"`);
    found += 1;
  }
  if (found === 0) throw new Error(`${path}: no rca crates to stamp`);
  writes.set(path, lock);
}

for (const [path, text] of writes) {
  writeFileSync(path, text);
  console.log(`${path} -> ${version}`);
}
