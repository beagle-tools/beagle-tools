import assert from "node:assert/strict";
import { test } from "node:test";

import { checkCoverage } from "./swift-coverage.mjs";

const report = [
  { name: "BeagleAPI", lineCoverage: 0.8182, executableLines: 132 },
  { name: "BeagleAPITests", lineCoverage: 0.87, executableLines: 293 },
];

test("at or above the floor passes", () => {
  const [lines, errors] = checkCoverage(report, { BeagleAPI: 80 });
  assert.deepEqual(errors, []);
  assert.match(lines[0], /^BeagleAPI: 81\.8% of 132 lines \(floor 80%\)$/);
});

test("below the floor fails", () => {
  const [, errors] = checkCoverage(report, { BeagleAPI: 85 });
  assert.match(errors[0], /BeagleAPI: line coverage 81\.8% is below its floor of 85%/);
});

test("a floored target missing from the report fails", () => {
  const [, errors] = checkCoverage(report, { BeagleUI: 10 });
  assert.match(errors[0], /^BeagleUI: no coverage reported/);
});
