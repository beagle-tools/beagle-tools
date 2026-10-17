#!/usr/bin/env node
// Fails when any app drifts from the shared standards in standards/.
// Every app must use the same lint levels, compiler flags, formatter
// settings, package manager and gate scripts — no per-app loosening.
import { existsSync, readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const read = (p) => readFileSync(join(root, p), "utf8");
const errors = [];
const fail = (msg) => errors.push(msg);

// The set of apps is closed: adding one means adding it here AND to
// .github/workflows/ci.yml, so nothing ships without a gate.
const RUST_WORKSPACES = [
  "utils",
  "tandem",
  "rca",
  "rca/desktop/src-tauri",
  "workflows",
  "workflows/desktop",
];
const FRONTENDS = ["tandem", "rca/desktop", "rca/web", "workflows/desktop"];
const REQUIRED_SCRIPTS = ["format:check", "lint", "typecheck", "test", "build"];
const SKIP_DIRS = new Set([".git", "target", "node_modules", "dist", ".astro", "gen"]);

function walk(dir, out = []) {
  for (const name of readdirSync(join(root, dir))) {
    const rel = dir ? `${dir}/${name}` : name;
    if (statSync(join(root, rel)).isDirectory()) {
      if (!SKIP_DIRS.has(name)) walk(rel, out);
    } else {
      out.push(rel);
    }
  }
  return out;
}
const files = walk("");
const norm = (s) => s.replace(/\r/g, "").trim();
// tsconfig files are JSONC: drop comments and trailing commas before parsing.
const readJsonc = (p) =>
  JSON.parse(
    read(p)
      .replace(/\/\*[\s\S]*?\*\//g, "")
      .replace(/^\s*\/\/.*$/gm, "")
      .replace(/,(\s*[}\]])/g, "$1"),
  );

// ---- Rust -----------------------------------------------------------------
const lints = norm(read("standards/cargo-lints.toml"));
const workspaceOf = new Map();
for (const ws of RUST_WORKSPACES) {
  const manifest = `${ws}/Cargo.toml`;
  if (!existsSync(join(root, manifest))) {
    fail(`${manifest}: missing (listed in RUST_WORKSPACES)`);
    continue;
  }
  const text = read(manifest);
  const start = text.indexOf("[workspace.lints.rust]");
  if (start === -1) {
    fail(`${manifest}: no [workspace.lints.*] block — copy standards/cargo-lints.toml`);
    continue;
  }
  // The block runs until the first table header that isn't a lints table.
  const rest = text.slice(start);
  const end = rest.search(/^\[(?!workspace\.lints\.)/m);
  const block = norm(end === -1 ? rest : rest.slice(0, end))
    .split("\n")
    .filter((l) => !l.trim().startsWith("#"))
    .join("\n");
  if (norm(block) !== lints) {
    fail(`${manifest}: [workspace.lints] differs from standards/cargo-lints.toml`);
  }
  if (!existsSync(join(root, ws, "Cargo.lock"))) fail(`${ws}/Cargo.lock: missing`);
  workspaceOf.set(ws, true);
}
for (const f of files.filter((p) => p.endsWith("Cargo.toml"))) {
  const text = read(f);
  if (/^\[package\]/m.test(text) && !/^\[lints\]\s*\nworkspace\s*=\s*true/m.test(text)) {
    fail(`${f}: package must opt into shared lints with \`[lints]\\nworkspace = true\``);
  }
  if (/^\[lints\.(rust|clippy)\]/m.test(text)) {
    fail(`${f}: per-package [lints.*] tables are not allowed; use the workspace block`);
  }
  if (/^\[workspace\]/m.test(text) && !workspaceOf.has(dirname(f))) {
    fail(`${f}: cargo workspace not listed in RUST_WORKSPACES (and CI)`);
  }
}
for (const f of files) {
  const base = f.split("/").pop();
  if (["rustfmt.toml", ".rustfmt.toml", "clippy.toml", ".clippy.toml"].includes(base)) {
    if (f !== "rustfmt.toml") fail(`${f}: per-app ${base} is not allowed; edit the root one`);
  }
  if (base === "rust-toolchain.toml" || base === "rust-toolchain") {
    if (f !== "rust-toolchain.toml") fail(`${f}: per-app toolchain pins are not allowed`);
  }
}

// ---- Frontend -------------------------------------------------------------
const baseTs = JSON.parse(read("standards/tsconfig.base.json")).compilerOptions;
const basePrettier = JSON.parse(read(".prettierrc.json"));
let packageManager;
for (const app of FRONTENDS) {
  if (!existsSync(join(root, app, "package.json"))) {
    fail(`${app}/package.json: missing`);
    continue;
  }
  const pkg = JSON.parse(read(`${app}/package.json`));
  packageManager ??= pkg.packageManager;
  if (!pkg.packageManager?.startsWith("pnpm@")) {
    fail(`${app}/package.json: packageManager must pin pnpm`);
  } else if (pkg.packageManager !== packageManager) {
    fail(`${app}/package.json: packageManager ${pkg.packageManager} != ${packageManager}`);
  }
  for (const s of REQUIRED_SCRIPTS) {
    if (!pkg.scripts?.[s]) fail(`${app}/package.json: missing "${s}" script`);
  }
  if (!pkg.scripts?.lint?.includes("--max-warnings 0")) {
    fail(`${app}/package.json: lint must run with --max-warnings 0`);
  }
  if (!existsSync(join(root, app, "pnpm-lock.yaml"))) fail(`${app}: missing pnpm-lock.yaml`);
  if (existsSync(join(root, app, "package-lock.json"))) {
    fail(`${app}: package-lock.json found — the monorepo uses pnpm only`);
  }

  if (!existsSync(join(root, app, "tsconfig.json"))) {
    fail(`${app}/tsconfig.json: missing`);
    continue;
  }
  const tsconfig = readJsonc(`${app}/tsconfig.json`);
  const ext = [tsconfig.extends ?? []].flat();
  const want = relative(join(root, app), join(root, "standards/tsconfig.base.json"));
  if (!ext.includes(want)) fail(`${app}/tsconfig.json: must extend "${want}"`);
  for (const key of Object.keys(tsconfig.compilerOptions ?? {})) {
    if (key in baseTs) fail(`${app}/tsconfig.json: redefines shared flag "${key}"`);
  }

  const eslint = existsSync(join(root, app, "eslint.config.js"))
    ? read(`${app}/eslint.config.js`)
    : "";
  if (!eslint.includes("standards/eslint.base.js")) {
    fail(`${app}/eslint.config.js: must build on standards/eslint.base.js`);
  }
  if (/["']off["']|:\s*0\b/.test(eslint)) {
    fail(`${app}/eslint.config.js: switches a rule off — shared rules can't be weakened`);
  }

  for (const name of [".prettierrc", ".prettierrc.json", ".prettierrc.js", "prettier.config.js"]) {
    if (!existsSync(join(root, app, name))) continue;
    // Allowed only to add plugins (e.g. Astro); shared keys must match root.
    const local = JSON.parse(read(`${app}/${name}`));
    for (const [k, v] of Object.entries(basePrettier)) {
      if (local[k] !== v) fail(`${app}/${name}: "${k}" must be ${JSON.stringify(v)}`);
    }
  }
}

if (errors.length) {
  for (const e of errors) console.error(`FAIL: ${e}`);
  console.error(`\n${errors.length} standards violation(s). See docs/CODING_STANDARDS.md.`);
  process.exit(1);
}
console.log("OK: every app matches the shared standards");
