// Tests for docs-only.mjs: prose docs skip the app gates; embedded markdown,
// frontend-package markdown, fixtures and code never do. Run: make test-scripts
import assert from "node:assert/strict";
import { test } from "node:test";

import { isDoc, isDocsOnly } from "./docs-only.mjs";

test("READMEs, guides and docs folders are docs", () => {
  for (const p of [
    "README.md",
    "CONTRIBUTING.md",
    "AGENTS.md",
    "docs/GETTING_STARTED.md",
    "rca/README.md",
    "rca/docs/reference.md",
    "tandem/docs/CLI.md",
    "agent/docs/manifests.md",
    "beagle/serve/README.md",
    "scout/tui/README.md",
    "ios/README.md",
    "rca/CHANGELOG.md",
  ]) {
    assert.ok(isDoc(p), p);
  }
});

test("markdown compiled into the binary is not docs", () => {
  for (const p of [
    "skills/beagle/SKILL.md",
    "skills/beagle-review/SKILL.md",
    "scout/tui/skills/scout/SKILL.md",
    "recap/tui/docs/AGENT_GUIDE.md",
    "standup/tui/docs/AGENT_GUIDE.md",
  ]) {
    assert.ok(!isDoc(p), p);
  }
});

test("markdown in a frontend package is not docs (prettier checks it)", () => {
  for (const p of ["agent/desktop/README.md", "rca/desktop/README.md", "rca/web/DEPLOY.md", "tandem/desktop/x.md"]) {
    assert.ok(!isDoc(p), p);
  }
});

test("fixtures, examples and code are not docs", () => {
  for (const p of [
    "rca/examples/rcas/2026-05-12-example-checkout-timeouts/summary.md",
    "agent/examples/demo/prompts/report.md",
    "rca/tui/src/main.rs",
    "docs/assets/beagle-logo.png",
    ".github/workflows/ci.yml",
    "Cargo.lock",
  ]) {
    assert.ok(!isDoc(p), p);
  }
});

test("docs only needs every file to be docs, and at least one file", () => {
  assert.ok(isDocsOnly(["README.md", "rca/docs/reference.md"]));
  assert.ok(!isDocsOnly(["README.md", "rca/tui/src/main.rs"]));
  assert.ok(!isDocsOnly([]));
});
