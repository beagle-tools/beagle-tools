// node:test suite for set-version.mjs. Runs against a temp copy of the real
// version files, so a manifest change that moves a version location breaks
// this test rather than a release. `make standards` runs it (cli-base has
// no frontend gate to host it).

import assert from "node:assert/strict";
import { cpSync, mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { setVersion, stampLock, stampTomlTable } from "./set-version.mjs";

const appDir = join(dirname(fileURLToPath(import.meta.url)), "..");

function tempCopy() {
  const dir = mkdtempSync(join(tmpdir(), "cli-set-version-"));
  for (const rel of ["Cargo.toml", "Cargo.lock"]) cpSync(join(appDir, rel), join(dir, rel));
  return dir;
}

/** Version of a path package (no `source =`) in a Cargo.lock. */
function lockVersion(text, name) {
  const block = text.split("[[package]]").find((b) => b.includes(`\nname = "${name}"\n`));
  assert.ok(block, `${name} missing from lockfile`);
  return /^version = "([^"]+)"$/m.exec(block)?.[1];
}

test("stamps the real manifest and lockfile", () => {
  const dir = tempCopy();
  setVersion(dir, "7.8.9");
  const read = (rel) => readFileSync(join(dir, rel), "utf8");
  assert.match(read("Cargo.toml"), /\[workspace\.package\]\nversion = "7\.8\.9"/);
  assert.equal(lockVersion(read("Cargo.lock"), "beagle-cli"), "7.8.9");
  // beagle-utils is a path dependency with its own version: untouched.
  assert.notEqual(lockVersion(read("Cargo.lock"), "beagle-utils"), "7.8.9");
});

test("rejects non X.Y.Z versions", () => {
  assert.throws(() => setVersion(tempCopy(), "1.2"), /not a X.Y.Z/);
});

test("a table without a version is reported, not silently skipped", () => {
  const { hit } = stampTomlTable('[package]\nname = "x"\n', "workspace.package", "1.0.0");
  assert.equal(hit, false);
});

test("registry packages with the same name are never stamped", () => {
  const lock = [
    "[[package]]",
    'name = "beagle-cli"',
    'version = "0.1.0"',
    'source = "registry+https://github.com/rust-lang/crates.io-index"',
    "",
  ].join("\n");
  const { text, stamped } = stampLock(lock, new Set(["beagle-cli"]), "9.9.9");
  assert.equal(stamped.size, 0);
  assert.equal(text, lock);
});
