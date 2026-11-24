#!/usr/bin/env node
// Decides which apps release at one commit of main, and at which version,
// then cleans up leftover unpublished tags and pushes the new
// `beagle-<app>-vX.Y.Z` tags. The decision rules live in lib.mjs (unit
// tested); this file only gathers the inputs and acts on the plan.
//
//   node scripts/release/plan.mjs [--after SHA] [--apps all|tandem,rca,...]
//        [--labels release:minor,...] [--dry-run]
//
// --after    the commit to release (default $GITHUB_SHA, else HEAD).
// --apps     force these apps as changed instead of diffing (dispatch).
// --labels   use these PR labels instead of asking GitHub.
// --dry-run  print the plan; no stale-main check, no tags pushed or deleted.
//
// Each app is diffed from its LAST PUBLISHED release tag (not this push's
// `before`), so a run that never ran — replaced in the concurrency queue,
// or cancelled — loses nothing: the next run releases it.
//
// Writes `<app>`, `<app>_version`, `<app>_tag` and `any` to $GITHUB_OUTPUT
// when it is set, and the plan as JSON to stdout.
import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { parseArgs } from "node:util";
import { APPS, changedApps, lastPublished, planApp, readVersion, tagFor, toOutputs } from "./lib.mjs";

const git = (...args) => execFileSync("git", args, { encoding: "utf8" }).trim();
const lines = (s) => s.split("\n").filter(Boolean);
const log = (msg) => process.stderr.write(`${msg}\n`);
const repo = process.env.GITHUB_REPOSITORY;

const { values: opts } = parseArgs({
  options: {
    after: { type: "string", default: process.env.GITHUB_SHA ?? "HEAD" },
    apps: { type: "string", default: "" },
    labels: { type: "string" },
    "dry-run": { type: "boolean", default: false },
  },
});
const dryRun = opts["dry-run"];
const after = git("rev-parse", `${opts.after}^{commit}`);

const gh = (...args) => execFileSync("gh", args, { encoding: "utf8" });

/** Every GitHub release: tag -> { id, draft }. Null when gh can't tell. */
function releases() {
  try {
    const out = gh("api", "--paginate", `repos/${repo}/releases?per_page=100`, "--jq", ".[] | [.tag_name, .id, .draft] | @tsv");
    return new Map(lines(out).map((l) => l.split("\t")).map(([tag, id, draft]) => [tag, { id, draft: draft === "true" }]));
  } catch (err) {
    if (!dryRun) throw err;
    log(`dry run: could not list releases (${err.message.split("\n")[0]}); every tag counts as published`);
    return null;
  }
}

const allTags = lines(git("tag", "-l", "beagle-*-v*"));
const headTags = lines(git("tag", "--points-at", after, "beagle-*-v*"));
const rel = releases();
const published = rel ? new Set([...rel].filter(([, r]) => !r.draft).map(([tag]) => tag)) : undefined;

/** The app's last published tag, or null (never released). */
function baseTag(app) {
  const v = lastPublished(app, allTags, published);
  return v ? tagFor(app, v) : null;
}

function detectChanged() {
  if (opts.apps) {
    const forced = opts.apps === "all" ? Object.keys(APPS) : opts.apps.split(",");
    const unknown = forced.filter((a) => !(a in APPS));
    if (unknown.length) throw new Error(`unknown app(s): ${unknown.join(", ")}`);
    log(`apps forced: ${forced.join(", ")}`);
    return new Set(forced);
  }
  const changed = new Set();
  for (const app of Object.keys(APPS)) {
    const base = baseTag(app);
    if (!base) {
      log(`${app}: no published release yet — counts as changed`);
      changed.add(app);
      continue;
    }
    const paths = lines(git("diff", "--name-only", base, after));
    const hit = changedApps(paths).includes(app);
    log(`${app}: ${paths.length} path(s) changed since ${base}${hit ? " — release-relevant" : ""}`);
    if (hit) changed.add(app);
  }
  return changed;
}

function isStale() {
  if (dryRun) return false;
  const tip = git("ls-remote", "origin", "refs/heads/main").split("\t")[0];
  if (tip !== after) log(`main is at ${tip}, not ${after}`);
  return tip !== after;
}

const labelCache = new Map();
/** Labels of the PRs that merged `sha`. */
function commitLabels(sha) {
  if (!labelCache.has(sha)) {
    try {
      const out = gh("api", `repos/${repo}/commits/${sha}/pulls`, "--jq", '[.[].labels[].name] | join("\\n")');
      labelCache.set(sha, lines(out));
    } catch (err) {
      log(`::warning::could not read PR labels of ${sha.slice(0, 12)} (${err.message.split("\n")[0]})`);
      labelCache.set(sha, []);
    }
  }
  return labelCache.get(sha);
}

/** Every label on the PRs merged since the app's last published release. */
function prLabels(app) {
  if (opts.labels !== undefined) return opts.labels.split(",").filter(Boolean);
  if (dryRun) return [];
  const base = baseTag(app);
  const shas = base ? lines(git("rev-list", "--first-parent", "--max-count=50", `${base}..${after}`)) : [after];
  return [...new Set(shas.flatMap(commitLabels))];
}

function fileVersion(app) {
  const { versionFile, kind } = APPS[app];
  return readVersion(kind, git("show", `${after}:${versionFile}`));
}

const changed = detectChanged();
const anyNeedsCheck = [...changed].some((app) => !headTags.some((t) => t.startsWith(`beagle-${app}-v`)));
const stale = anyNeedsCheck && isStale();

const plans = Object.keys(APPS).map((app) => {
  const labels = changed.has(app) && !stale ? prLabels(app) : [];
  if (labels.length) log(`${app}: PR labels ${labels.join(", ")}`);
  return planApp({
    app,
    changed: changed.has(app),
    headTags,
    allTags,
    published,
    labels,
    stale,
    fileVersion: changed.has(app) ? fileVersion(app) : undefined,
  });
});
for (const p of plans) {
  log(`${p.app}: ${p.release ? `release ${p.tag}` : "skip"} (${p.reason})`);
}

// Leftovers first: a new tag may reuse the version of a deleted one.
const cleanup = plans.flatMap((p) => (p.release ? (p.cleanup ?? []) : []));
if (cleanup.length && !dryRun) {
  for (const tag of cleanup) {
    const r = rel?.get(tag);
    if (r) gh("api", "-X", "DELETE", `repos/${repo}/releases/${r.id}`);
    git("tag", "-d", tag);
  }
  git("push", "origin", ...cleanup.map((t) => `:refs/tags/${t}`));
  log(`deleted unpublished ${cleanup.join(", ")} (their drafts too)`);
} else if (cleanup.length) {
  log(`dry run — would delete unpublished ${cleanup.join(", ")} and their drafts`);
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
