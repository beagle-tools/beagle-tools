// Stamp a release version into the files the bundler and updater read.
// Used by .github/workflows/tandem-release.yml (repo root): node scripts/set-version.mjs 0.1.4
import { readFileSync, writeFileSync } from "node:fs";

const version = process.argv[2];
if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
  console.error(`usage: node scripts/set-version.mjs <major.minor.patch> (got ${version ?? ""})`);
  process.exit(1);
}

for (const path of ["src-tauri/tauri.conf.json", "package.json"]) {
  const json = JSON.parse(readFileSync(path, "utf8"));
  json.version = version;
  writeFileSync(path, JSON.stringify(json, null, 2) + "\n");
  console.log(`${path} -> ${version}`);
}
