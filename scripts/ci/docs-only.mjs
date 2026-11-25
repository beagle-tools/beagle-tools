#!/usr/bin/env node
// Whether a change touches only prose docs, so CI can skip every app gate
// (the `changes` job in .github/workflows/ci.yml). `standards` and `gate`
// still run, so docs still get the affiliation guard.
//
//   node scripts/ci/docs-only.mjs --base <git-ref>         # diff <ref>...HEAD
//   node scripts/ci/docs-only.mjs --changed <file> [...]
//
// stdout: `true` or `false`; also written as `only=` to $GITHUB_OUTPUT when
// set. The list is deliberately narrow: anything not listed here is treated
// as code and runs the gates. docs/RELEASING.md has the matching release rule.
import { execFileSync } from "node:child_process";
import { appendFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

/** Markdown files that are only ever read by people and agents. */
const DOC_NAMES = new Set(["README.md", "AGENTS.md", "CLAUDE.md", "CHANGELOG.md", "CONTRIBUTING.md"]);

/**
 * Frontend packages run `prettier --check .`, which formats their markdown
 * too, so a docs change inside one still needs its gate.
 */
const FRONTENDS = ["tandem/desktop/", "rca/desktop/", "rca/web/", "agent/desktop/"];

/** Markdown compiled into a binary with include_str!: code, not docs. */
const EMBEDDED = ["skills/", "scout/tui/skills/", "recap/tui/docs/", "standup/tui/docs/"];

/** Whether one changed path is prose docs. */
export function isDoc(path) {
  if (FRONTENDS.some((p) => path.startsWith(p))) return false;
  if (EMBEDDED.some((p) => path.startsWith(p))) return false;
  if (!path.endsWith(".md")) return false;
  const parts = path.split("/");
  if (DOC_NAMES.has(parts[parts.length - 1])) return true;
  // The repo's docs/ and each tool's own docs/ (rca/docs, tandem/docs, …).
  return parts[0] === "docs" || parts[1] === "docs";
}

/** True when there is at least one change and every change is docs. */
export function isDocsOnly(paths) {
  return paths.length > 0 && paths.every(isDoc);
}

function main(argv) {
  let base = null;
  const changed = [];
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--base") base = argv[++i];
    else if (argv[i] === "--changed") changed.push(argv[++i]);
    else throw new Error(`unexpected argument: ${argv[i]}`);
  }
  if (!base && changed.length === 0) throw new Error("usage: docs-only.mjs (--base <ref> | --changed <file>...)");
  const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
  const paths = base
    ? execFileSync("git", ["diff", "--name-only", `${base}...HEAD`], { cwd: root, encoding: "utf8" })
        .split("\n")
        .filter(Boolean)
    : changed;
  const only = isDocsOnly(paths);
  const out = process.env.GITHUB_OUTPUT;
  if (out) appendFileSync(out, `only=${only}\n`);
  process.stderr.write(`${paths.length} changed file(s); docs only: ${only}\n`);
  process.stdout.write(`${only}\n`);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2));
}
