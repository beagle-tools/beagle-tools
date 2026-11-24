// Stamp a release version into every place Tandem records one:
//   package.json, src-tauri/tauri.conf.json, src-tauri/Cargo.toml's
//   [workspace.package] version (the desktop shell's own workspace), each
//   crates/*/Cargo.toml [package] version, and those crates' entries in the
//   repo-root Cargo.lock and src-tauri/Cargo.lock (so `cargo build --locked`
//   keeps working).
//
// Run from tandem/ by release CI only; the result is never committed.
//   node scripts/set-version.mjs 0.1.4
import { existsSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`usage: node scripts/set-version.mjs <major.minor.patch> (got ${version ?? ""})`);
  process.exit(1);
}

const SHELL = "src-tauri/Cargo.toml";
// The repo-root workspace holds the crates; the shell keeps its own lock.
const LOCKS = ["../Cargo.lock", "src-tauri/Cargo.lock"];
const CRATE_MANIFESTS = readdirSync("crates", { withFileTypes: true })
  .filter((entry) => entry.isDirectory() && existsSync(join("crates", entry.name, "Cargo.toml")))
  .map((entry) => join("crates", entry.name, "Cargo.toml"))
  .sort();

/** Replace exactly one match of `pattern` in `text`, or fail loudly. */
function replaceOne(path, text, pattern, replacement) {
  const matches = text.match(new RegExp(pattern.source, pattern.flags + "g"));
  if (matches?.length !== 1) {
    throw new Error(`${path}: expected one version to stamp, found ${matches?.length ?? 0}`);
  }
  return text.replace(pattern, replacement);
}

/** The `[section]` body of a TOML file: from its header to the next header. */
function section(text, header) {
  const lines = text.split("\n");
  const start = lines.indexOf(header);
  if (start < 0) return null;
  const rest = lines.slice(start + 1).join("\n");
  const end = rest.search(/^\[/m);
  return end < 0 ? rest : rest.slice(0, end);
}

/** Stamp `version = "…"` inside one table of a manifest. */
function stampTable(path, text, header) {
  const body = section(text, header);
  if (body === null) throw new Error(`${path}: no ${header} table`);
  const stamped = replaceOne(path, body, /^version = "[^"]*"$/m, `version = "${version}"`);
  return text.replace(body, stamped);
}

// Validate and compute everything first so a failure leaves no file half-stamped.
const writes = new Map();

// JSON: rewrite the top-level "version" in place so formatting survives.
for (const path of ["package.json", "src-tauri/tauri.conf.json"]) {
  const text = replaceOne(
    path,
    readFileSync(path, "utf8"),
    /^ {2}"version": "[^"]*"/m,
    `  "version": "${version}"`,
  );
  if (JSON.parse(text).version !== version)
    throw new Error(`${path}: top-level version was not stamped`);
  writes.set(path, text);
}

// The shell's [workspace.package] version, inherited by the `tandem` package.
const shell = readFileSync(SHELL, "utf8");
writes.set(SHELL, stampTable(SHELL, shell, "[workspace.package]"));
const names = ["tandem"];

// Each crate's own [package] version.
for (const path of CRATE_MANIFESTS) {
  const text = readFileSync(path, "utf8");
  const name = /^name = "([^"]+)"$/m.exec(section(text, "[package]") ?? "")?.[1];
  if (!name) throw new Error(`${path}: no [package] name`);
  writes.set(path, stampTable(path, text, "[package]"));
  names.push(name);
}

// Lockfiles: every tandem crate each one records.
for (const path of LOCKS) {
  let lock = readFileSync(path, "utf8");
  let found = 0;
  for (const name of names) {
    const escaped = name.replace(/[-.]/g, "\\$&");
    const pattern = new RegExp(
      `^(\\[\\[package\\]\\]\\nname = "${escaped}"\\nversion = )"[^"]*"$`,
      "m",
    );
    if (!pattern.test(lock)) continue;
    lock = replaceOne(path, lock, pattern, `$1"${version}"`);
    found += 1;
  }
  if (found === 0) throw new Error(`${path}: no tandem crates to stamp`);
  writes.set(path, lock);
}

for (const [path, text] of writes) {
  writeFileSync(path, text);
  console.log(`${path} -> ${version}`);
}
