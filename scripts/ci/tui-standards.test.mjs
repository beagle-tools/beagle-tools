// Tests for tui-standards.mjs: the real repo passes, and each kind of
// drift (doc table, unregistered TUI crate, keymap test that ignores the
// shared data) fails.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkTuiStandards, docKeymapRows } from "./tui-standards.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const readReal = (p) => readFileSync(join(root, p), "utf8");
const data = JSON.parse(readReal("standards/tui-standards.json"));
const realFiles = [
  ...Object.keys(data.tuis).map((d) => `${d}/Cargo.toml`),
  ...Object.values(data.tuis),
];

/** `read` over the real repo with some files replaced. */
const readWith = (overrides) => (p) => (p in overrides ? overrides[p] : readReal(p));

test("the repo as it stands passes", () => {
  assert.deepEqual(checkTuiStandards(readReal, realFiles), []);
});

test("the doc table parses into job/keys rows", () => {
  const rows = docKeymapRows(readReal("docs/CODING_STANDARDS.md"));
  assert.deepEqual(rows[0], ["Move", data.keymap[0].doc]);
  assert.equal(rows.length, data.keymap.length);
});

test("editing the doc table alone fails", () => {
  const doc = readReal("docs/CODING_STANDARDS.md").replace("| Reload | `r` |", "| Reload | `R` |");
  const errors = checkTuiStandards(readWith({ "docs/CODING_STANDARDS.md": doc }), realFiles);
  assert.match(errors.join("\n"), /keymap table differs/);
});

test("dropping a GitHub write from the doc fails", () => {
  const doc = readReal("docs/CODING_STANDARDS.md").replace("`M` merge", "`M` combine");
  const errors = checkTuiStandards(readWith({ "docs/CODING_STANDARDS.md": doc }), realFiles);
  assert.match(errors.join("\n"), /`M` merge is not listed/);
});

test("a new crate on beagle-tui must register", () => {
  const files = [...realFiles, "acme/widgets/Cargo.toml"];
  const read = readWith({
    "acme/widgets/Cargo.toml": '[package]\nname = "widgets"\n\n[dependencies]\nbeagle-tui.workspace = true\n',
  });
  assert.match(checkTuiStandards(read, files).join("\n"), /acme\/widgets\/Cargo.toml: depends on beagle-tui/);
});

test("a keymap test that does not walk the shared data fails", () => {
  const [dir, testFile] = Object.entries(data.tuis)[0];
  const read = readWith({ [testFile]: "#[test]\nfn rca_follows_the_standard_keymap() {}\n" });
  assert.match(checkTuiStandards(read, realFiles).join("\n"), new RegExp(`${testFile}: needs`));
  assert.ok(dir);
});
