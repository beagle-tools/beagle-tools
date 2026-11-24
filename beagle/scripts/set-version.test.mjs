// node:test suite for set-version.mjs. Runs against a temp copy of the real
// version files, so a manifest change that moves a version location breaks
// this test rather than a release. Run: node --test beagle/scripts/
// (`make test-scripts` and the CI standards job run it).

import assert from "node:assert/strict";
import { cpSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, relative } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { memberManifests, packageName, setVersion, stampLock, stampTomlTable } from "./set-version.mjs";

const appDir = join(dirname(fileURLToPath(import.meta.url)), "..");

/** A temp repo root: the CRATES' manifests at their real relative paths, plus the root Cargo.lock. */
function tempCopy() {
  const root = mkdtempSync(join(tmpdir(), "beagle-set-version-"));
  const dir = join(root, "beagle");
  for (const rel of memberManifests(appDir).map((p) => relative(appDir, p))) {
    cpSync(join(appDir, rel), join(dir, rel), { recursive: true });
  }
  cpSync(join(appDir, "..", "Cargo.lock"), join(root, "Cargo.lock"));
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

  for (const path of memberManifests(dir)) {
    assert.match(readFileSync(path, "utf8"), /^\[package\]\nname = "[^"]+"\n(?:description = .*\n)?version = "7\.8\.9"\n/m, path);
  }
  for (const name of ["beagle-cli", "beagle-recap", "beagle-scout", "beagle-standup", "beagle-serve"]) {
    assert.equal(lockVersion(read("../Cargo.lock"), name), "7.8.9", name);
  }
  // Path dependencies from other apps keep their own versions.
  for (const name of ["beagle", "tandem-cli", "agentctl", "beagle-utils", "beagle-tui"]) {
    assert.notEqual(lockVersion(read("../Cargo.lock"), name), "7.8.9", name);
  }
});

test("rejects anything that is not X.Y.Z", () => {
  const dir = tempCopy();
  for (const bad of ["1.2", "v1.2.3", "1.2.3-rc1", ""]) {
    assert.throws(() => setVersion(dir, bad), /not a X\.Y\.Z/);
  }
});

test("fails loudly, and writes nothing, when a crate lost its version", () => {
  const dir = tempCopy();
  const path = join(dir, "Cargo.toml");
  const lockBefore = readFileSync(join(dir, "../Cargo.lock"), "utf8");
  writeFileSync(path, readFileSync(path, "utf8").replace(/^version = .*$/m, ""));
  assert.throws(() => setVersion(dir, "1.0.0"), /no \[package\] version/);
  assert.equal(readFileSync(join(dir, "../Cargo.lock"), "utf8"), lockBefore);
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
    '[[package]]\nname = "beagle-cli"\nversion = "0.1.0"\n\n',
    '[[package]]\nname = "beagle-cli"\nversion = "9.9.9"\nsource = "registry+https://github.com/rust-lang/crates.io-index"\n',
  ].join("");
  const res = stampLock(lock, new Set(["beagle-cli"]), "1.2.3");
  assert.match(res.text, /name = "beagle-cli"\nversion = "1\.2\.3"\n\n/);
  assert.match(res.text, /version = "9\.9\.9"\nsource/);
  assert.deepEqual([...res.stamped], ["beagle-cli"]);
});

test("packageName reads the [package] table only", () => {
  assert.equal(packageName('[package]\nname = "beagle-cli"\n'), "beagle-cli");
  assert.equal(packageName('[workspace]\nname = "x"\n'), null);
});
