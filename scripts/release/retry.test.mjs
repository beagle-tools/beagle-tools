// Tests for re-running release jobs that never got a runner:
//   node --test scripts/release/retry.test.mjs
import assert from "node:assert/strict";
import { test } from "node:test";

import { MAX_ATTEMPTS, decideRetry, neverAcquired } from "./retry.mjs";

// The shape GitHub reports for a job the macOS shortage cancelled.
const stuck = { name: "tandem-desktop / Tandem (macos-intel)", conclusion: "cancelled", runner_name: "", steps: [] };
const built = { name: "rca-cli / beagle-rca (x86_64)", conclusion: "success", runner_name: "GitHub Actions 12", steps: [{}] };
const broke = { name: "cli-build / beagle-cli (aarch64)", conclusion: "failure", runner_name: "GitHub Actions 3", steps: [{}, {}] };

test("only a job that ended without a runner or steps never got one", () => {
  assert.equal(neverAcquired(stuck), true);
  assert.equal(neverAcquired({ ...stuck, conclusion: "failure", runner_name: null }), true);
  assert.equal(neverAcquired(built), false);
  assert.equal(neverAcquired(broke), false);
  assert.equal(neverAcquired({ ...stuck, conclusion: "skipped" }), false);
});

test("re-runs a run whose only problem was jobs without a runner", () => {
  const d = decideRetry({ attempt: 1, jobs: [built, stuck], pendingRuns: 0 });
  assert.equal(d.rerun, true);
  assert.match(d.reason, /Tandem \(macos-intel\)/);
});

test("a real build failure is not retried", () => {
  assert.equal(decideRetry({ attempt: 1, jobs: [built, broke], pendingRuns: 0 }).rerun, false);
});

test("a newer pending run releases the apps instead", () => {
  const d = decideRetry({ attempt: 1, jobs: [stuck], pendingRuns: 1 });
  assert.equal(d.rerun, false);
  assert.match(d.reason, /pending/);
});

test("stops after the last attempt", () => {
  assert.equal(decideRetry({ attempt: MAX_ATTEMPTS, jobs: [stuck], pendingRuns: 0 }).rerun, false);
  assert.equal(decideRetry({ attempt: MAX_ATTEMPTS - 1, jobs: [stuck], pendingRuns: 0 }).rerun, true);
});
