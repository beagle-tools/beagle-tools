#!/usr/bin/env node
// Re-runs a finished release run's jobs that GitHub never gave a runner
// (the macOS shortage: "The job was not acquired by Runner of type hosted
// even after multiple attempts"). Called by .github/workflows/release-retry.yml
// when a release run ends cancelled or failed.
//
//   node scripts/release/retry.mjs <run id> [--dry-run]
//
// Needs GH_TOKEN and GITHUB_REPOSITORY. The rule lives in decideRetry
// (tested by retry.test.mjs).
import { execFileSync } from "node:child_process";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";

/** Most attempts a run gets (the first one included). */
export const MAX_ATTEMPTS = 3;

/** A job that ended without ever starting on a runner. */
export function neverAcquired(job) {
  const ended = job.conclusion === "cancelled" || job.conclusion === "failure";
  return ended && !job.runner_name && (job.steps ?? []).length === 0;
}

/**
 * Whether to re-run a finished release run's failed jobs:
 *  - only jobs that never got a runner count; a real build failure stays red;
 *  - not when another release run is queued or running: it diffs from each
 *    app's last published release, so it releases these apps too (and
 *    replaces this run's leftover drafts);
 *  - at most MAX_ATTEMPTS attempts per run.
 */
export function decideRetry({ attempt, jobs, pendingRuns }) {
  const stuck = jobs.filter(neverAcquired);
  if (stuck.length === 0) return { rerun: false, reason: "every job got a runner; nothing to retry" };
  if (pendingRuns > 0) {
    return { rerun: false, reason: `${pendingRuns} newer release run(s) pending; they release these apps` };
  }
  if (attempt >= MAX_ATTEMPTS) {
    return { rerun: false, reason: `already at attempt ${attempt}; re-run by hand once runners are back` };
  }
  return { rerun: true, reason: `re-running ${stuck.length} job(s) that never got a runner: ${stuck.map((j) => j.name).join(", ")}` };
}

const invokedDirectly = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (invokedDirectly) {
  const { values, positionals } = parseArgs({ options: { "dry-run": { type: "boolean", default: false } }, allowPositionals: true });
  const runId = positionals[0];
  if (!runId) throw new Error("usage: retry.mjs <run id> [--dry-run]");
  const repo = process.env.GITHUB_REPOSITORY;
  const gh = (...args) => execFileSync("gh", args, { encoding: "utf8" });
  const run = JSON.parse(gh("api", `repos/${repo}/actions/runs/${runId}`));
  const jobs = JSON.parse(gh("api", "--paginate", "--slurp", `repos/${repo}/actions/runs/${runId}/attempts/${run.run_attempt}/jobs?per_page=100`))
    .flatMap((page) => page.jobs);
  const pendingRuns = ["queued", "in_progress", "waiting", "pending", "requested"]
    .map((status) => JSON.parse(gh("api", `repos/${repo}/actions/workflows/release.yml/runs?status=${status}&per_page=1`)).total_count)
    .reduce((a, b) => a + b, 0);
  const decision = decideRetry({ attempt: run.run_attempt, jobs, pendingRuns });
  console.log(decision.reason);
  if (decision.rerun && !values["dry-run"]) {
    gh("run", "rerun", String(runId), "--failed", "-R", repo);
    console.log(`re-ran the failed jobs of run ${runId}`);
  } else if (!decision.rerun && jobs.some(neverAcquired)) {
    console.log(`::warning::run ${runId} has jobs that never got a runner and was not re-run: ${decision.reason}`);
  }
}
