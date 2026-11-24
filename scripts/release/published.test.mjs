// Tests for planning from each app's last PUBLISHED release: drafts left by
// a cancelled or failed run never count as released, are deleted, and their
// version numbers are reused. Run: node --test scripts/release/
import assert from "node:assert/strict";
import { test } from "node:test";

import { lastPublished, planApp, unpublishedAbove } from "./lib.mjs";

// Today's shape: tandem published 0.3.0, then three runs left drafts.
const allTags = [
  "beagle-tandem-v0.2.11",
  "beagle-tandem-v0.3.0",
  "beagle-tandem-v0.3.1",
  "beagle-tandem-v0.3.2",
  "beagle-tandem-v0.3.3",
  "beagle-rca-v0.34.3",
];
const published = new Set(["beagle-tandem-v0.2.11", "beagle-tandem-v0.3.0", "beagle-rca-v0.34.3"]);
const base = {
  app: "tandem",
  changed: true,
  headTags: [],
  allTags,
  published,
  labels: [],
  stale: false,
  fileVersion: "0.0.1",
};

test("the last published version ignores drafts", () => {
  assert.equal(lastPublished("tandem", allTags, published), "0.3.0");
  assert.equal(lastPublished("tandem", allTags, undefined), "0.3.3");
  assert.equal(lastPublished("cli", allTags, published), null);
});

test("unpublished tags above the last published one are leftovers", () => {
  assert.deepEqual(unpublishedAbove("tandem", allTags, published), [
    "beagle-tandem-v0.3.3",
    "beagle-tandem-v0.3.2",
    "beagle-tandem-v0.3.1",
  ]);
  assert.deepEqual(unpublishedAbove("rca", allTags, published), []);
  // An old draft below the last published release is not touched.
  const old = [...allTags, "beagle-tandem-v0.2.5"];
  assert.ok(!unpublishedAbove("tandem", old, published).includes("beagle-tandem-v0.2.5"));
  // Without release data nothing counts as unpublished.
  assert.deepEqual(unpublishedAbove("tandem", allTags, undefined), []);
});

test("a release bumps the last published version and replaces the drafts", () => {
  const p = planApp(base);
  assert.deepEqual([p.action, p.version, p.tag], ["bump", "0.3.1", "beagle-tandem-v0.3.1"]);
  assert.deepEqual(p.cleanup, ["beagle-tandem-v0.3.3", "beagle-tandem-v0.3.2", "beagle-tandem-v0.3.1"]);
  assert.match(p.reason, /patch bump from 0\.3\.0; replaces unpublished/);
  assert.equal(planApp({ ...base, labels: ["release:minor"] }).version, "0.4.0");
});

test("an app that never published starts from its version file and clears its drafts", () => {
  const tags = ["beagle-cli-v0.1.0"];
  const p = planApp({ ...base, app: "cli", allTags: tags, published: new Set(), fileVersion: "0.1.0" });
  assert.deepEqual([p.action, p.tag, p.cleanup], ["first", "beagle-cli-v0.1.0", ["beagle-cli-v0.1.0"]]);
});

test("a draft tag at HEAD is a re-run: reused, nothing deleted", () => {
  const p = planApp({ ...base, headTags: ["beagle-tandem-v0.3.3"] });
  assert.deepEqual([p.action, p.version, p.cleanup], ["reuse", "0.3.3", undefined]);
});

test("skips never delete anything", () => {
  assert.equal(planApp({ ...base, changed: false }).cleanup, undefined);
  assert.equal(planApp({ ...base, stale: true }).cleanup, undefined);
});
