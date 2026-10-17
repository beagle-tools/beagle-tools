#!/usr/bin/env node
// Which packages of one cargo workspace a change affects. scripts/ci/rust.sh
// runs this when AFFECTED_BASE is set, then tests only what it prints.
//
//   node scripts/ci/affected.mjs <workspace-dir> --base <git-ref>
//   node scripts/ci/affected.mjs <workspace-dir> --changed <file> [--changed <file>...]
//   add --json for machine-readable output
//
// stdout (one line): `all`, or the affected package names separated by
// spaces, or nothing when no package in this workspace is affected.
// stderr: what was selected and why. The rules live in affected-lib.mjs.
import { execFileSync } from "node:child_process";
import { appendFileSync, existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { includeTargets, selectPackages } from "./affected-lib.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const rel = (abs) => relative(root, abs).split(sep).join("/");
const log = (msg) => process.stderr.write(`${msg}\n`);

function parseArgs(argv) {
  const opts = { ws: null, base: null, changed: null, json: false };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--base") opts.base = argv[++i];
    else if (arg === "--changed") (opts.changed ??= []).push(argv[++i]);
    else if (arg === "--json") opts.json = true;
    else if (!opts.ws) opts.ws = arg.replace(/\/+$/, "");
    else throw new Error(`unexpected argument: ${arg}`);
  }
  if (!opts.ws || (!opts.base && !opts.changed)) {
    throw new Error("usage: affected.mjs <workspace-dir> (--base <ref> | --changed <file>...) [--json]");
  }
  return opts;
}

const git = (...args) =>
  execFileSync("git", args, { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });

/** Files changed since the merge-base with `base`, including uncommitted and untracked ones. */
function changedSince(base) {
  let mergeBase;
  try {
    mergeBase = git("merge-base", base, "HEAD").trim();
  } catch {
    return { error: `base ${base} is unavailable (no merge-base with HEAD)` };
  }
  const tracked = git("diff", "--name-only", "--no-renames", mergeBase).split("\n");
  const untracked = git("ls-files", "--others", "--exclude-standard").split("\n");
  const files = [...new Set([...tracked, ...untracked].filter(Boolean))];
  return { files, mergeBase };
}

const metadataCache = new Map();
/** `cargo metadata --no-deps` for the workspace containing `manifestDir` (offline, cached). */
function metadata(manifestDir) {
  const out = execFileSync(
    "cargo",
    ["metadata", "--no-deps", "--format-version", "1", "--manifest-path", join(manifestDir, "Cargo.toml")],
    { cwd: root, encoding: "utf8", stdio: ["ignore", "pipe", "inherit"], maxBuffer: 64 << 20 },
  );
  const meta = JSON.parse(out);
  metadataCache.set(meta.workspace_root, meta);
  return meta;
}
function metadataFor(manifestDir) {
  for (const [wsRoot, meta] of metadataCache) {
    if (meta.packages.some((p) => dirname(p.manifest_path) === manifestDir)) return [wsRoot, meta];
  }
  const meta = metadata(manifestDir);
  return [meta.workspace_root, meta];
}

/** Rust files under `dir`, skipping build output. */
function rustFiles(dir, out = []) {
  for (const name of readdirSync(dir)) {
    if (name === "target" || name === "node_modules" || name.startsWith(".")) continue;
    const path = join(dir, name);
    if (statSync(path).isDirectory()) rustFiles(path, out);
    else if (name.endsWith(".rs")) out.push(path);
  }
  return out;
}

/** Inputs outside `absDir` that its sources compile in via include_str!/include_bytes!. */
function externalIncludes(absDir) {
  const extra = new Set();
  for (const file of rustFiles(absDir)) {
    for (const target of includeTargets(readFileSync(file, "utf8"))) {
      const abs = resolve(dirname(file), target);
      if (!abs.startsWith(absDir + sep)) extra.add(rel(abs));
    }
  }
  return [...extra];
}

/** The package graph for workspace `ws`: its members plus every path dep they reach. */
function buildGraph(ws) {
  const wsAbs = join(root, ws);
  const meta = metadata(wsAbs);
  const members = new Set(meta.workspace_members.map((id) => {
    const pkg = meta.packages.find((p) => p.id === id);
    return rel(dirname(pkg.manifest_path));
  }));
  const nodes = new Map();
  const pending = meta.packages.map((p) => ({ pkg: p, wsRoot: meta.workspace_root }));
  while (pending.length > 0) {
    const { pkg, wsRoot } = pending.shift();
    const absDir = dirname(pkg.manifest_path);
    const dir = rel(absDir);
    if (nodes.has(dir)) continue;
    const extra = externalIncludes(absDir);
    // An external dep inherits from its own workspace's manifest.
    if (resolve(wsRoot) !== resolve(wsAbs) && resolve(wsRoot) !== absDir) {
      extra.push(`${rel(wsRoot)}/Cargo.toml`);
    }
    const deps = [];
    for (const dep of pkg.dependencies.filter((d) => d.path)) {
      const depAbs = resolve(dep.path);
      deps.push(rel(depAbs));
      if (nodes.has(rel(depAbs)) || !existsSync(join(depAbs, "Cargo.toml"))) continue;
      const [depWsRoot, depMeta] = metadataFor(depAbs);
      const depPkg = depMeta.packages.find((p) => dirname(p.manifest_path) === depAbs);
      if (depPkg) pending.push({ pkg: depPkg, wsRoot: depWsRoot });
    }
    nodes.set(dir, { name: pkg.name, dir, deps, extra });
  }
  return { members, nodes };
}

function report(ws, result, why) {
  const lines = [];
  if (result.all) {
    lines.push(`[${ws}] testing the whole workspace: ${result.reason}`);
  } else if (result.packages.length === 0) {
    lines.push(`[${ws}] no package affected by ${why} — skipping tests`);
  } else {
    lines.push(`[${ws}] testing ${result.packages.length} affected package(s) (${why}):`);
    for (const p of result.packages) lines.push(`  - ${p.name}: ${p.reason}`);
  }
  for (const line of lines) log(line);
  if (process.env.GITHUB_STEP_SUMMARY) {
    appendFileSync(process.env.GITHUB_STEP_SUMMARY, `### Test selection\n\n\`\`\`\n${lines.join("\n")}\n\`\`\`\n`);
  }
}

function main() {
  const opts = parseArgs(process.argv.slice(2));
  if (!existsSync(join(root, opts.ws, "Cargo.toml"))) {
    throw new Error(`${opts.ws}/Cargo.toml not found (expected a cargo workspace dir)`);
  }
  let changed = opts.changed;
  let why = "the given files";
  if (!changed) {
    const diff = changedSince(opts.base);
    if (diff.error) {
      const result = { all: true, reason: diff.error };
      report(opts.ws, result, "");
      process.stdout.write(opts.json ? `${JSON.stringify(result)}\n` : "all\n");
      return;
    }
    changed = diff.files;
    why = `${changed.length} file(s) changed since ${opts.base} (${diff.mergeBase.slice(0, 10)})`;
  }
  const result = selectPackages({ changed, ws: opts.ws, graph: buildGraph(opts.ws) });
  report(opts.ws, result, why);
  if (opts.json) process.stdout.write(`${JSON.stringify(result)}\n`);
  else process.stdout.write(result.all ? "all\n" : `${result.packages.map((p) => p.name).join(" ")}\n`);
}

try {
  main();
} catch (err) {
  log(`affected.mjs: ${err instanceof Error ? err.message : String(err)}`);
  process.exit(2);
}
