#!/usr/bin/env node
// Decides which apps release for one push to main, and at which version,
// then creates + pushes the `beagle-<app>-vX.Y.Z` tags. The decision rules
// live in lib.mjs (unit tested); this file only gathers the inputs.
//
//   node scripts/release/plan.mjs [--before SHA] [--after SHA]
//        [--apps all|tandem,rca,...] [--labels release:minor,...] [--dry-run]
//
// --before   the push's previous main SHA (github.event.before). Missing,
//            all-zeros or not in the clone -> every app counts as changed.
// --after    the commit to release (default $GITHUB_SHA, else HEAD).
// --apps     force these apps as changed instead of diffing (dispatch).
// --labels   use these PR labels instead of asking GitHub.
// --dry-run  print the plan; no stale-main check, no gh, no tags pushed.
//
// Writes `<app>`, `<app>_version`, `<app>_tag` and `any` to $GITHUB_OUTPUT
// when it is set, and the plan as JSON to stdout.
import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { APPS, changedApps, isUnknownBefore, planApp, readVersion, toOutputs } from "./lib.mjs";

const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const lines = (s) => s.split("\n").filter(Boolean);
const log = (msg) => process.stderr.write(`${msg}\n`);

const { values: opts } = parseArgs({
  options: {
    before: { type: "string", default: process.env.BEFORE ?? "" },
    after: { type: "string", default: process.env.GITHUB_SHA ?? "HEAD" },
    apps: { type: "string", default: "" },
    labels: { type: "string" },
    "dry-run": { type: "boolean", default: false },
  },
});
const dryRun = opts["dry-run"];
const after = git("rev-parse", `${opts.after}^{commit}`);

function commitExists(sha) {
  try {
    execFileSync("git", ["cat-file", "-e", `${sha}^{commit}`], { stdio: "ignore" });
    return true;
  } catch {
    return false;
  }
}

function detectChanged() {
  if (opts.apps) {
    const forced = opts.apps === "all" ? Object.keys(APPS) : opts.apps.split(",");
    const unknown = forced.filter((a) => !(a in APPS));
    if (unknown.length) throw new Error(`unknown app(s): ${unknown.join(", ")}`);
    log(`apps forced: ${forced.join(", ")}`);
    return forced;
  }
  if (isUnknownBefore(opts.before) || !commitExists(opts.before)) {
    log(`before "${opts.before}" unknown — treating every app as changed`);
    return Object.keys(APPS);
  }
  const paths = lines(git("diff", "--name-only", opts.before, after));
  log(`${paths.length} path(s) changed in ${opts.before.slice(0, 12)}..${after.slice(0, 12)}`);
  return changedApps(paths);
}

function isStale() {
  if (dryRun) return false;
  const tip = git("ls-remote", "origin", "refs/heads/main").split("\t")[0];
  if (tip !== after) log(`main is at ${tip}, not ${after}`);
  return tip !== after;
}

function prLabels() {
  if (opts.labels !== undefined) return opts.labels.split(",").filter(Boolean);
  if (dryRun) return [];
  const repo = process.env.GITHUB_REPOSITORY;
  try {
    const out = execFileSync(
      "gh",
      ["api", `repos/${repo}/commits/${after}/pulls`, "--jq", '[.[].labels[].name] | join("\\n")'],
      { encoding: "utf8" },
    );
    return lines(out);
  } catch (err) {
    log(`::warning::could not read PR labels (${err.message.split("\n")[0]}); bumping patch`);
    return [];
  }
}

function fileVersion(app) {
  const { versionFile, kind } = APPS[app];
  return readVersion(kind, git("show", `${after}:${versionFile}`));
}

const changed = new Set(detectChanged());
const allTags = lines(git("tag", "-l", "beagle-*-v*"));
const headTags = lines(git("tag", "--points-at", after, "beagle-*-v*"));
const anyNeedsCheck = Object.keys(APPS).some(
  (app) => changed.has(app) && !headTags.some((t) => t.startsWith(`beagle-${app}-v`)),
);
const stale = anyNeedsCheck && isStale();
const labels = anyNeedsCheck ? prLabels() : [];
if (labels.length) log(`PR labels: ${labels.join(", ")}`);

const plans = Object.keys(APPS).map((app) =>
  planApp({
    app,
    changed: changed.has(app),
    headTags,
    allTags,
    labels,
    stale,
    fileVersion: changed.has(app) ? fileVersion(app) : undefined,
  }),
);
for (const p of plans) {
  log(`${p.app}: ${p.release ? `release ${p.tag}` : "skip"} (${p.reason})`);
}

const newTags = plans.filter((p) => p.release && p.action !== "reuse").map((p) => p.tag);
if (newTags.length && !dryRun) {
  for (const tag of newTags) git("tag", tag, after);
  git("push", "origin", ...newTags.map((t) => `refs/tags/${t}`));
  log(`pushed ${newTags.join(", ")}`);
} else if (newTags.length) {
  log(`dry run — would tag ${after.slice(0, 12)} with ${newTags.join(", ")}`);
}

if (process.env.GITHUB_OUTPUT) {
  appendFileSync(process.env.GITHUB_OUTPUT, `${toOutputs(plans).join("\n")}\n`);
}
process.stdout.write(`${JSON.stringify({ after, dryRun, plans }, null, 2)}\n`);
