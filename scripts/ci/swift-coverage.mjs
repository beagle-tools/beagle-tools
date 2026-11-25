#!/usr/bin/env node
// The Swift coverage floor (docs/CODING_STANDARDS.md, Swift): fails when a
// library's line coverage drops below its floor in standards/swift/coverage.json.
//
//   node scripts/ci/swift-coverage.mjs <dir> <xccov-report.json>
//
// The report is `xcrun xccov view --report --only-targets --json <xcresult>`.
// checkCoverage is pure, so scripts/ci/swift-coverage.test.mjs can feed it.
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/**
 * Compares an xccov target report with the floors for one package. Returns
 * [lines, errors]: one line per floored target, and the failures.
 */
export function checkCoverage(report, floors) {
  const lines = [];
  const errors = [];
  const byName = new Map(report.map((t) => [t.name, t]));
  for (const [name, floor] of Object.entries(floors)) {
    const target = byName.get(name);
    if (!target) {
      errors.push(`${name}: no coverage reported (is it still a target, and tested?)`);
      continue;
    }
    const percent = Math.floor(target.lineCoverage * 1000) / 10;
    lines.push(`${name}: ${percent}% of ${target.executableLines} lines (floor ${floor}%)`);
    if (percent < floor) {
      errors.push(`${name}: line coverage ${percent}% is below its floor of ${floor}% — add tests; never lower the floor`);
    }
  }
  return [lines, errors];
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dir, reportPath] = process.argv.slice(2);
  if (!dir || !reportPath) {
    console.error("usage: swift-coverage.mjs <dir> <xccov-report.json>");
    process.exit(2);
  }
  const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
  const floors = JSON.parse(readFileSync(join(root, "standards/swift/coverage.json"), "utf8"));
  if (!floors[dir]) {
    console.error(`standards/swift/coverage.json: no floors for ${dir}`);
    process.exit(1);
  }
  const [lines, errors] = checkCoverage(JSON.parse(readFileSync(reportPath, "utf8")), floors[dir]);
  for (const l of lines) console.log(`    ${l}`);
  for (const e of errors) console.error(`FAIL: ${e}`);
  process.exit(errors.length ? 1 : 0);
}
