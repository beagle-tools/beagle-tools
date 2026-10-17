// Tests for set-version.mjs. Each test runs the script against a temp copy
// of the real version files, so the checkout itself is never modified.
//   node --test scripts/set-version.test.mjs   (from rca/; also run by
//   rca/desktop's `pnpm test`, so the frontend CI gate covers it)
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";

const appDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const script = join(appDir, "scripts", "set-version.mjs");
const JSON_FILES = ["desktop/package.json", "desktop/src-tauri/tauri.conf.json", "web/package.json"];
const MANIFESTS = [
  "cli/Cargo.toml",
  "crates/beagle-agent/Cargo.toml",
  "crates/beagle-agentd/Cargo.toml",
  "desktop/src-tauri/Cargo.toml",
];
const LOCKS = ["Cargo.lock", "desktop/src-tauri/Cargo.lock"];
const FILES = [...JSON_FILES, ...MANIFESTS, ...LOCKS];
const CRATES = {
  "Cargo.lock": ["beagle", "beagle-agent", "beagle-agentd"],
  "desktop/src-tauri/Cargo.lock": ["beagle", "beagle-desktop"],
};

const temps = [];
after(() => {
  for (const dir of temps) rmSync(dir, { recursive: true, force: true });
});

function copyApp() {
  const dir = mkdtempSync(join(tmpdir(), "rca-set-version-"));
  temps.push(dir);
  for (const file of FILES) {
    mkdirSync(dirname(join(dir, file)), { recursive: true });
    cpSync(join(appDir, file), join(dir, file));
  }
  return dir;
}

const run = (cwd, ...args) => spawnSync(process.execPath, [script, ...args], { cwd, encoding: "utf8" });
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

  for (const file of JSON_FILES) assert.equal(JSON.parse(read(dir, file)).version, "4.5.6", file);
  for (const file of MANIFESTS) assert.match(read(dir, file), /^\[package\]\nname = "[^"]+"\nversion = "4\.5\.6"\n/m, file);

  for (const [file, crates] of Object.entries(CRATES)) {
    const before = lockVersions(read(appDir, file));
    const stamped = lockVersions(read(dir, file));
    for (const name of crates) assert.equal(stamped.get(name), "4.5.6", `${file} ${name}`);
    for (const [name, version] of before) {
      if (!crates.includes(name)) assert.equal(stamped.get(name), version, `${file} ${name} untouched`);
    }
  }
});

test("only the version lines change", () => {
  const dir = copyApp();
  assert.equal(run(dir, "7.8.9").status, 0);
  for (const file of FILES) {
    const a = read(appDir, file).split("\n");
    const b = read(dir, file).split("\n");
    assert.equal(a.length, b.length, file);
    const changed = a.filter((line, i) => line !== b[i] && !line.includes('"7.8.9"'));
    const expected = CRATES[file]?.length ?? 1;
    assert.ok(changed.length <= expected, `${file}: ${changed.join(" | ")}`);
    for (const [i, line] of b.entries()) {
      if (line !== a[i]) assert.match(line, /version/, `${file}: unexpected change ${line}`);
    }
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

test("fails without writing anything when a location is missing", () => {
  const dir = copyApp();
  rmSync(join(dir, "desktop/src-tauri/Cargo.lock"));
  const result = run(dir, "1.2.3");
  assert.notEqual(result.status, 0);
  for (const file of FILES.filter((f) => f !== "desktop/src-tauri/Cargo.lock")) {
    assert.equal(read(dir, file), read(appDir, file), `${file} untouched`);
  }
});
