// Unit tests for the release-planning rules:
//   node --test scripts/release/plan.test.mjs
import assert from "node:assert/strict";
import { test } from "node:test";
import {
  appVersions,
  bumpFromLabels,
  bumpVersion,
  changedApps,
  isReleaseRelevant,
  isUnknownBefore,
  planApp,
  readVersion,
  toOutputs,
} from "./lib.mjs";

test("app paths release only their app", () => {
  assert.deepEqual(changedApps(["tandem/src/App.tsx"]), ["tandem"]);
  assert.deepEqual(changedApps(["rca/cli/src/main.rs", "workflows/Cargo.lock"]), [
    "rca",
    "workflows",
  ]);
});

test("shared inputs release every app", () => {
  const all = ["tandem", "rca", "workflows", "cli"];
  assert.deepEqual(changedApps(["utils/src/fs.rs"]), all);
  assert.deepEqual(changedApps(["rust-toolchain.toml"]), all);
});

test("the base CLI releases from cli-base/ as app cli", () => {
  assert.deepEqual(changedApps(["cli-base/src/main.rs"]), ["cli"]);
  assert.deepEqual(changedApps(["cli-base/Cargo.lock"]), ["cli"]);
  assert.deepEqual(changedApps(["cli-base/AGENTS.md", "cli-base/docs/x.txt"]), []);
  assert.equal(isReleaseRelevant("cli", "cli/x.rs"), false);
});

test("docs-only changes never release", () => {
  assert.equal(isReleaseRelevant("tandem", "tandem/README.md"), false);
  assert.equal(isReleaseRelevant("rca", "rca/docs/arch.txt"), false);
  assert.equal(isReleaseRelevant("rca", "rca/desktop/AGENTS.md"), false);
  assert.equal(isReleaseRelevant("tandem", "utils/README.md"), false);
  assert.deepEqual(changedApps(["docs/RELEASING.md", "README.md", "AGENTS.md"]), []);
});

test("skills rca embeds release rca, even as markdown", () => {
  assert.deepEqual(changedApps(["skills/beagle/SKILL.md"]), ["rca"]);
  assert.deepEqual(changedApps(["skills/beagle-review/SKILL.md"]), ["rca"]);
  assert.deepEqual(changedApps(["skills/beagle-create-issue/SKILL.md"]), []);
});

test("repo tooling outside the apps does not release", () => {
  assert.deepEqual(
    changedApps([".github/workflows/release.yml", "scripts/release/plan.mjs", "website/x.ts"]),
    [],
  );
  // A nested docs/ directory below the app root still releases.
  assert.equal(isReleaseRelevant("rca", "rca/web/docs/page.astro"), true);
  // Prefix match must be on a whole path segment.
  assert.equal(isReleaseRelevant("rca", "rcadir/x.rs"), false);
});

test("unknown before SHAs", () => {
  assert.equal(isUnknownBefore(""), true);
  assert.equal(isUnknownBefore(undefined), true);
  assert.equal(isUnknownBefore("0000000000000000000000000000000000000000"), true);
  assert.equal(isUnknownBefore("d9759cd"), false);
});

test("labels pick the bump, patch by default", () => {
  assert.equal(bumpFromLabels([]), "patch");
  assert.equal(bumpFromLabels(["bug", "release:patch"]), "patch");
  assert.equal(bumpFromLabels(["release:minor"]), "minor");
  assert.equal(bumpFromLabels(["release:minor", "release:major"]), "major");
});

test("semver bumps", () => {
  assert.equal(bumpVersion("0.29.0", "patch"), "0.29.1");
  assert.equal(bumpVersion("0.29.7", "minor"), "0.30.0");
  assert.equal(bumpVersion("1.9.9", "major"), "2.0.0");
  assert.throws(() => bumpVersion("1.0", "patch"));
});

test("tag versions sort numerically and ignore other apps", () => {
  const tags = [
    "beagle-rca-v0.9.0",
    "beagle-rca-v0.10.0",
    "beagle-rca-v0.10.0-rc.1",
    "beagle-rca-latest",
    "beagle-tandem-v5.0.0",
    "v1.0.0",
  ];
  assert.deepEqual(appVersions("rca", tags), ["0.10.0", "0.9.0"]);
});

test("reads every kind of version file", () => {
  assert.equal(readVersion("json", '{"productName":"Tandem","version":"0.0.1"}'), "0.0.1");
  const pkg =
    '[package]\nname = "beagle"\nversion = "0.29.0"\n\n[dependencies]\nx = { version = "1" }\n';
  assert.equal(readVersion("cargo-package", pkg), "0.29.0");
  const ws = '[workspace]\nmembers = []\n\n[workspace.package]\nversion = "0.1.0"\n';
  assert.equal(readVersion("cargo-workspace", ws), "0.1.0");
  assert.throws(() => readVersion("cargo-workspace", pkg));
});

const base = {
  app: "rca",
  changed: true,
  headTags: [],
  allTags: [],
  labels: [],
  stale: false,
  fileVersion: "0.29.0",
};

test("first release uses the version file", () => {
  const p = planApp(base);
  assert.equal(p.action, "first");
  assert.equal(p.tag, "beagle-rca-v0.29.0");
  assert.throws(() => planApp({ ...base, fileVersion: "next" }));
});

test("later releases bump the highest tag by label", () => {
  const allTags = ["beagle-rca-v0.29.0", "beagle-rca-v0.29.3", "beagle-tandem-v9.0.0"];
  assert.equal(planApp({ ...base, allTags }).tag, "beagle-rca-v0.29.4");
  assert.equal(planApp({ ...base, allTags, labels: ["release:minor"] }).version, "0.30.0");
});

test("a tag at HEAD is reused, even when stale or unchanged", () => {
  const headTags = ["beagle-rca-v0.29.3", "beagle-tandem-v0.0.4"];
  const p = planApp({ ...base, headTags, changed: false, stale: true });
  assert.deepEqual([p.action, p.version], ["reuse", "0.29.3"]);
});

test("unchanged or stale apps skip", () => {
  assert.equal(planApp({ ...base, changed: false }).release, false);
  assert.equal(planApp({ ...base, stale: true }).release, false);
});

test("outputs carry one block per app plus any", () => {
  const out = toOutputs([planApp(base), planApp({ ...base, app: "tandem", changed: false })]);
  assert.deepEqual(out, [
    "rca=true",
    "rca_version=0.29.0",
    "rca_tag=beagle-rca-v0.29.0",
    "tandem=false",
    "tandem_version=",
    "tandem_tag=",
    "any=true",
  ]);
});
