// Tests for set-version.mjs. Each test runs the script against a temp copy
// of the real version files, so the checkout itself is never modified.
//   node --test scripts/set-version.test.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const appDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const script = join(appDir, "scripts", "set-version.mjs");
const MEMBERS = ["src-tauri", "crates/tandem-core", "crates/tandem-github"]
  .concat(["crates/tandem-cache", "crates/tandem-agents"])
  .map((dir) => join(dir, "Cargo.toml"));
const FILES = ["package.json", "src-tauri/tauri.conf.json", "Cargo.toml", "Cargo.lock", ...MEMBERS];
const CRATES = ["tandem", "tandem-core", "tandem-github", "tandem-cache", "tandem-agents"];

const temps = [];
after(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function copyApp() {
  const dir = mkdtempSync(join(tmpdir(), "tandem-set-version-"));
  temps.push(dir);
  for (const file of FILES) cpSync(join(appDir, file), join(dir, file));
  return dir;
}

const run = (cwd, ...args) =>
  spawnSync(process.execPath, [script, ...args], { cwd, encoding: "utf8" });
const read = (dir, file) => readFileSync(join(dir, file), "utf8");

/** `name -> version` for every package in a Cargo.lock. */
function lockVersions(lock) {
  const out = new Map();
  for (const m of lock.matchAll(/\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"/g)) {
    out.set(m[1], m[2]);
  }
  return out;
}

test("stamps every version location", () => {
  const dir = copyApp();
  const result = run(dir, "4.5.6");
  assert.equal(result.status, 0, result.stderr);

  assert.equal(JSON.parse(read(dir, "package.json")).version, "4.5.6");
  assert.equal(JSON.parse(read(dir, "src-tauri/tauri.conf.json")).version, "4.5.6");
  assert.match(read(dir, "Cargo.toml"), /\[workspace\.package\]\nversion = "4\.5\.6"\n/);

  const before = lockVersions(read(appDir, "Cargo.lock"));
  const stamped = lockVersions(read(dir, "Cargo.lock"));
  for (const name of CRATES) assert.equal(stamped.get(name), "4.5.6", name);
  for (const [name, version] of before) {
    if (!CRATES.includes(name)) assert.equal(stamped.get(name), version, `${name} untouched`);
  }
});

test("only the version lines change", () => {
  const dir = copyApp();
  assert.equal(run(dir, "7.8.9").status, 0);
  for (const file of ["package.json", "src-tauri/tauri.conf.json", "Cargo.toml", "Cargo.lock"]) {
    const a = read(appDir, file).split("\n");
    const b = read(dir, file).split("\n");
    assert.equal(a.length, b.length, file);
    const changed = a.filter((line, i) => line !== b[i]);
    const expected = file === "Cargo.lock" ? CRATES.length : 1;
    assert.equal(changed.length, expected, `${file}: ${changed.join(" | ")}`);
  }
});

test("rejects a missing or malformed version without touching files", () => {
  const dir = copyApp();
  for (const bad of [[], ["1.2"], ["v1.2.3"], ["1.2.3-rc1"]]) {
    const result = run(dir, ...bad);
    assert.equal(result.status, 1, `args ${JSON.stringify(bad)}`);
    assert.match(result.stderr, /usage:/);
  }
  for (const file of FILES) assert.equal(read(dir, file), read(appDir, file), file);
});
