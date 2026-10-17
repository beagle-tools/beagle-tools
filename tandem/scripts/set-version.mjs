// Stamp a release version into every place Tandem records one:
//   package.json, src-tauri/tauri.conf.json, Cargo.toml's
//   [workspace.package] version, and the workspace members' entries in
//   Cargo.lock (so `cargo build --locked` keeps working).
//
// Run from tandem/ by release CI only; the result is never committed.
//   node scripts/set-version.mjs 0.1.4
import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`usage: node scripts/set-version.mjs <major.minor.patch> (got ${version ?? ""})`);
  process.exit(1);
}

/** Replace exactly one match of `pattern` in `path`, or fail loudly. */
function stamp(path, pattern, replacement) {
  const text = readFileSync(path, "utf8");
  const matches = text.match(new RegExp(pattern.source, pattern.flags + "g"));
  if (matches?.length !== 1) {
    throw new Error(`${path}: expected one version to stamp, found ${matches?.length ?? 0}`);
  }
  writeFileSync(path, text.replace(pattern, replacement));
}

/** The `[section]` body of a TOML file: from its header to the next header. */
function section(text, header) {
  const lines = text.split("\n");
  const start = lines.indexOf(header);
  if (start < 0) return null;
  const rest = lines.slice(start + 1).join("\n");
  const end = rest.search(/^\[/m);
  return end < 0 ? rest : rest.slice(0, end);
}

// JSON: rewrite the top-level "version" in place so formatting survives.
for (const path of ["package.json", "src-tauri/tauri.conf.json"]) {
  stamp(path, /^ {2}"version": "[^"]*"/m, `  "version": "${version}"`);
  if (JSON.parse(readFileSync(path, "utf8")).version !== version) {
    throw new Error(`${path}: top-level version was not stamped`);
  }
  console.log(`${path} -> ${version}`);
}

// Cargo.toml: [workspace.package] version, inherited by every member.
const manifest = readFileSync("Cargo.toml", "utf8");
const pkgSection = section(manifest, "[workspace.package]");
if (pkgSection === null || !/^version = "[^"]*"$/m.test(pkgSection)) {
  throw new Error("Cargo.toml: no [workspace.package] version to stamp");
}
const stampedSection = pkgSection.replace(/^version = "[^"]*"$/m, `version = "${version}"`);
writeFileSync("Cargo.toml", manifest.replace(pkgSection, stampedSection));
console.log(`Cargo.toml [workspace.package] -> ${version}`);

// Cargo.lock: the members that inherit the workspace version.
const membersBlock = /^members = \[([^\]]*)\]/m.exec(manifest);
if (!membersBlock) throw new Error("Cargo.toml: no [workspace] members list");
const members = [...membersBlock[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
const crateNames = members.flatMap((dir) => {
  const toml = readFileSync(join(dir, "Cargo.toml"), "utf8");
  const pkg = section(toml, "[package]") ?? "";
  const name = /^name = "([^"]+)"/m.exec(pkg)?.[1];
  return name && /^version\.workspace = true/m.test(pkg) ? [name] : [];
});
for (const name of crateNames) {
  const escaped = name.replace(/[-.]/g, "\\$&");
  stamp(
    "Cargo.lock",
    new RegExp(`^(\\[\\[package\\]\\]\\nname = "${escaped}"\\nversion = )"[^"]*"$`, "m"),
    `$1"${version}"`,
  );
  console.log(`Cargo.lock ${name} -> ${version}`);
}
