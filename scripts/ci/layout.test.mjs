// Tests for layout.mjs: the real repo passes, and each way of breaking
// docs/LAYOUT.md (a crate in a stray folder, a tool without its folder, a
// non-part folder inside a tool) fails. Run: make test-scripts
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkLayout, toolNames } from "./layout.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const readReal = (p) => readFileSync(join(root, p), "utf8");
const realFiles = execFileSync("git", ["ls-files"], { cwd: root, encoding: "utf8" }).split("\n").filter(Boolean);
const check = (extra = [], read = readReal) => checkLayout(read, [...realFiles, ...extra]);

test("the repo as it stands passes", () => {
  assert.deepEqual(check(), []);
});

test("tool names come from TOOLS, without serve", () => {
  const names = toolNames(readReal("beagle/src/tools.rs"));
  assert.ok(names.includes("scout") && names.includes("agent"), names.join(", "));
  assert.ok(!names.includes("serve"));
});

test("a crate under a stray top-level folder fails", () => {
  const errors = check(["foo/crates/x/Cargo.toml", "foo/crates/x/src/lib.rs"]);
  assert.equal(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /^foo\/: not a `beagle` tool/);
});

test("a crates/ folder inside a tool fails, and so does its crate", () => {
  const errors = check(["agent/crates/x/Cargo.toml"]);
  assert.ok(errors.some((e) => e.startsWith("agent/crates/: not allowed in a tool folder")), errors.join("\n"));
  assert.ok(errors.some((e) => e.startsWith("agent/crates/x/Cargo.toml: a crate here")), errors.join("\n"));
});

test("a crate in an allowed folder but the wrong place fails", () => {
  const errors = check(["scout/tui/nested/Cargo.toml"]);
  assert.deepEqual(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /^scout\/tui\/nested\/Cargo\.toml: a crate here/);
});

test("crates in lib/<name>, daemon/ and desktop/ pass", () => {
  assert.deepEqual(check(["scout/lib/store/Cargo.toml", "scout/daemon/Cargo.toml", "scout/desktop/Cargo.toml"]), []);
});

test("a stray file at a tool's top level fails", () => {
  const errors = check(["scout/package.json"]);
  assert.equal(errors.length, 1);
  assert.match(errors[0], /^scout\/package\.json: not allowed in a tool folder/);
});

test("a new tool needs its folder", () => {
  const tools = readReal("beagle/src/tools.rs").replace(
    'name: "agent",',
    'name: "agent",\n        about: "",\n        run: x,\n    },\n    Tool {\n        name: "notes",',
  );
  const errors = checkLayout((p) => (p === "beagle/src/tools.rs" ? tools : readReal(p)), realFiles);
  assert.equal(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /^notes\/tui\/Cargo\.toml: missing/);
});

test("the Beagle iPhone app lives at ios/, not in a tool", () => {
  assert.deepEqual(check(["ios/Beagle/Tools/Recap/RecapView.swift"]), []);
  const errors = check(["scout/ios/Scout.xcodeproj/project.pbxproj", "scout/ios/Scout/App.swift"]);
  assert.equal(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /^scout\/ios\/: not allowed — the one iPhone app is ios\//);
});

test("utils/ holds the Rust crates and the Swift package, nothing else", () => {
  assert.deepEqual(check(["utils/swift/Package.swift", "utils/tui/src/lib.rs", "utils/src/lib.rs"]), []);
  const errors = check(["utils/kotlin/build.gradle"]);
  assert.equal(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /^utils\/kotlin: not allowed in utils\//);
});
