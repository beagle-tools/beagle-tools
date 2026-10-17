// node:test suite for set-version.mjs. Runs against a temp copy of the real
// version files, so a manifest change that moves a version location breaks
// this test rather than a release. Run: node --test scripts/
// (the Agentdesk `test` script runs it, so the frontend gate covers it).

import assert from "node:assert/strict";
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { setVersion, stampJson, stampLock, stampTomlTable, workspaceMembers } from "./set-version.mjs";

const appDir = join(dirname(fileURLToPath(import.meta.url)), "..");

const FILES = [
  "Cargo.toml",
  "Cargo.lock",
  "crates/agent-core/Cargo.toml",
  "crates/agentd/Cargo.toml",
  "crates/agentctl/Cargo.toml",
  "crates/agenttop/Cargo.toml",
  "desktop/Cargo.toml",
  "desktop/Cargo.lock",
  "desktop/package.json",
  "desktop/src-tauri/Cargo.toml",
  "desktop/src-tauri/tauri.conf.json",
];

function tempCopy() {
  const dir = mkdtempSync(join(tmpdir(), "wf-set-version-"));
  for (const rel of FILES) cpSync(join(appDir, rel), join(dir, rel), { recursive: true });
  return dir;
}

/** Version of a path package (no `source =`) in a Cargo.lock. */
function lockVersion(text, name) {
  const block = text.split("[[package]]").find((b) => b.includes(`\nname = "${name}"\n`));
  assert.ok(block, `${name} missing from lockfile`);
  assert.ok(!/^source = /m.test(block), `${name} is not a path package`);
  return /^version = "([^"]+)"$/m.exec(block)?.[1];
}

test("stamps every version location of the real tree", () => {
  const dir = tempCopy();
  setVersion(dir, "7.8.9");
  const read = (rel) => readFileSync(join(dir, rel), "utf8");

  for (const rel of ["Cargo.toml", "desktop/Cargo.toml"]) {
    assert.match(read(rel), /\[workspace\.package\]\nversion = "7\.8\.9"/, rel);
  }
  assert.equal(JSON.parse(read("desktop/package.json")).version, "7.8.9");
  assert.equal(JSON.parse(read("desktop/src-tauri/tauri.conf.json")).version, "7.8.9");
  for (const name of ["agent-core", "agentd", "agentctl", "agenttop"]) {
    assert.equal(lockVersion(read("Cargo.lock"), name), "7.8.9", name);
  }
  for (const name of ["agent-core", "agentdesk"]) {
    assert.equal(lockVersion(read("desktop/Cargo.lock"), name), "7.8.9", name);
  }
  // beagle-utils is a path dependency too, but it is not ours to version.
  assert.notEqual(lockVersion(read("desktop/Cargo.lock"), "beagle-utils"), "7.8.9");
});

test("rejects anything that is not X.Y.Z", () => {
  const dir = tempCopy();
  for (const bad of ["1.2", "v1.2.3", "1.2.3-rc1", ""]) {
    assert.throws(() => setVersion(dir, bad), /not a X\.Y\.Z/);
  }
});

test("fails loudly when a workspace lost its version", () => {
  const dir = tempCopy();
  const path = join(dir, "desktop/Cargo.toml");
  writeFileSync(path, readFileSync(path, "utf8").replace(/^version = .*$/m, ""));
  assert.throws(() => setVersion(dir, "1.0.0"), /no \[workspace\.package\] version/);
});

test("stampTomlTable only touches the named table", () => {
  const text = '[package]\nversion = "1.0.0"\n\n[dependencies]\nfoo = { version = "1" }\nversion = "x"\n';
  const res = stampTomlTable(text, "package", "2.0.0");
  assert.equal(res.hit, true);
  assert.equal(res.text, text.replace('"1.0.0"', '"2.0.0"'));
  assert.equal(stampTomlTable(text, "workspace.package", "2.0.0").hit, false);
});

test("stampLock skips registry packages with the same name", () => {
  const lock = [
    "version = 4\n\n",
    '[[package]]\nname = "agentd"\nversion = "0.1.0"\n\n',
    '[[package]]\nname = "agentd"\nversion = "9.9.9"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n',
  ].join("");
  const res = stampLock(lock, new Set(["agentd"]), "1.2.3");
  assert.match(res.text, /name = "agentd"\nversion = "1\.2\.3"\n\n/);
  assert.match(res.text, /version = "9\.9\.9"\nsource/);
  assert.deepEqual([...res.stamped], ["agentd"]);
});

test("stampJson keeps formatting and only sets the top-level version", () => {
  const text = '{\n  "name": "x",\n  "version": "0.1.0",\n  "deps": { "a": { "version": "3" } }\n}\n';
  assert.equal(stampJson(text, "4.5.6"), text.replace('"0.1.0"', '"4.5.6"'));
  assert.throws(() => stampJson('{\n  "name": "x"\n}\n', "1.0.0"), /no top-level version/);
});

test("workspaceMembers reads the members array", () => {
  assert.deepEqual(workspaceMembers('[workspace]\nmembers = [\n  "a",\n  "b/c",\n]\n'), ["a", "b/c"]);
  assert.deepEqual(workspaceMembers("[package]\n"), []);
});
