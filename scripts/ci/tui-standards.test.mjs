// Tests for tui-standards.mjs: the real repo passes, and each kind of
// drift (doc table, unregistered TUI crate, keymap test that ignores the
// shared data) fails.
import assert from "node:assert/strict";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkTuiStandards, docKeymapRows } from "./tui-standards.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const readReal = (p) => readFileSync(join(root, p), "utf8");
const data = JSON.parse(readReal("standards/tui-standards.json"));
/** Every file under `dir` (repo-relative). */
const walk = (dir) =>
  readdirSync(join(root, dir)).flatMap((name) => {
    const rel = `${dir}/${name}`;
    return statSync(join(root, rel)).isDirectory() ? walk(rel) : [rel];
  });
const realFiles = Object.keys(data.tuis).flatMap((d) => [`${d}/Cargo.toml`, ...walk(`${d}/src`)]);

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

test("a TUI that draws its own help or footer fails", () => {
  const dir = "tandem/crates/tandem-tui";
  const view = `${dir}/src/ui/view.rs`;
  const own = readReal(view).replaceAll("chrome::draw_help(", "draw_help(").replaceAll("chrome::draw_footer(", "draw_footer(");
  const errors = checkTuiStandards(readWith({ [view]: own }), realFiles).join("\n");
  assert.match(errors, /tandem-tui: draw the \? help with beagle_tui::chrome::draw_help/);
  assert.match(errors, /tandem-tui: draw the footer with beagle_tui::chrome::draw_footer/);
});

test("a reserved GitHub-write capital bound to something else fails", () => {
  const f = "cli/crates/scout/src/ui/keys.rs";
  const src = readReal(f).replace("KeyCode::Char('?')", "KeyCode::Char('?' | 'M')");
  const errors = checkTuiStandards(readWith({ [f]: src }), realFiles);
  assert.match(errors.join("\n"), /binds `M`, a capital reserved for a GitHub write/);
});

test("a declared GitHub write may use its capital", () => {
  assert.ok(readReal("cli/crates/recap/src/ui/keys.rs").includes("Char('M')"));
  assert.deepEqual(checkTuiStandards(readReal, realFiles), []);
});

test("declaring a non-write capital fails", () => {
  const json = JSON.parse(readReal("standards/tui-standards.json"));
  json.tui_github_writes["rca/cli"] = ["T"];
  const errors = checkTuiStandards(
    readWith({ "standards/tui-standards.json": JSON.stringify(json) }),
    realFiles,
  );
  assert.match(errors.join("\n"), /declares T, which is not a GitHub write/);
});
