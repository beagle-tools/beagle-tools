// Keeps the release planner and CI in step with what `beagle` really links:
// a crate compiled into it must re-run its gate and cut a release when it
// changes, or a fix lands on main and never reaches `beagle update`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { LINKED_PREFIXES, isReleaseRelevant } from "./lib.mjs";
import { linkedCrateDirs } from "./linked.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
// Inside beagle/ (the cli app's dir), cli/ (tools not yet moved; linked and
// filtered wholesale) or the shared crates, which already release and gate cli.
const covered = (dir) =>
  dir === "beagle" || dir.startsWith("beagle/") || dir.startsWith("cli/") || dir === "utils" || dir.startsWith("utils/");
const outside = linkedCrateDirs(root, "beagle").filter((d) => !covered(d));

test("the link walk finds the crates beagle is known to link", () => {
  for (const dir of ["rca/cli", "tandem/crates/tandem-cli", "tandem/crates/tandem-tui"]) {
    assert.ok(outside.includes(dir), `${dir} missing from ${outside.join(", ")}`);
  }
});

test("every crate beagle links releases beagle when it changes", () => {
  for (const dir of outside) {
    assert.ok(
      isReleaseRelevant("cli", `${dir}/src/lib.rs`),
      `${dir} is linked into beagle but not in LINKED_PREFIXES.cli (${LINKED_PREFIXES.cli.join(", ")})`,
    );
  }
});

test("every crate beagle links re-runs the beagle gate in CI", () => {
  const ci = readFileSync(join(root, ".github/workflows/ci.yml"), "utf8");
  const filter = ci.slice(ci.indexOf("            beagle:"), ci.indexOf("\n\n", ci.indexOf("            beagle:")));
  for (const dir of outside) {
    assert.ok(filter.includes(`"${dir}/**"`), `${dir}/** missing from ci.yml's beagle filter`);
  }
});
