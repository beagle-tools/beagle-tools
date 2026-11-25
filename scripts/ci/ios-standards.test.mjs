import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkIos } from "./ios-standards.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const repoRead = (p) => readFileSync(join(root, p), "utf8");

const PBX = "scout/ios/Scout.xcodeproj/project.pbxproj";
const SCHEME = "scout/ios/Scout.xcodeproj/xcshareddata/xcschemes/Scout.xcscheme";
const GOOD_PBX = "CODE_SIGN_STYLE = Automatic;\nPRODUCT_BUNDLE_IDENTIFIER = dev.example.app;\n";
const GOOD_CI =
  'filters: |\n  scout-ios:\n    - "scout/ios/**"\n    - "utils/swift/**"\n' +
  'run: ./scripts/ci/ios.sh utils/swift\nrun: ./scripts/ci/ios.sh scout/ios\n';
const GOOD_MAKE = "check-ios:\n\t./scripts/ci/ios.sh utils/swift;\n\t./scripts/ci/ios.sh scout/ios;\n";

function fake(overrides = {}) {
  const tree = {
    ".github/workflows/ci.yml": GOOD_CI,
    Makefile: GOOD_MAKE,
    [PBX]: GOOD_PBX,
    [SCHEME]: "<Scheme/>",

    "utils/swift/Package.swift": "// swift-tools-version: 6.0",
    ...overrides,
  };
  const files = Object.keys(tree).filter((f) => tree[f] !== null);
  return [(p) => tree[p], files];
}

test("the repo's own iPhone apps pass", () => {
  const files = [PBX, SCHEME, "utils/swift/Package.swift"];
  assert.deepEqual(checkIos(repoRead, files), []);
});

test("a well-formed app passes", () => {
  assert.deepEqual(checkIos(...fake()), []);
});

test("a committed team ID fails, an empty one passes", () => {
  assert.match(checkIos(...fake({ [PBX]: 'DEVELOPMENT_TEAM = AB12CD34EF;\n' }))[0], /never commit a team ID/);
  assert.deepEqual(checkIos(...fake({ [PBX]: 'DEVELOPMENT_TEAM = "";\n' })), []);
});

test("manual signing or a committed profile fails", () => {
  assert.match(checkIos(...fake({ [PBX]: "CODE_SIGN_STYLE = Manual;\n" }))[0], /automatic/);
  assert.match(checkIos(...fake({ [PBX]: 'PROVISIONING_PROFILE_SPECIFIER = "My Profile";\n' }))[0], /profile/);
});

test("an app without its CI job, path filter, make target or scheme fails", () => {
  const errors = checkIos(...fake({ ".github/workflows/ci.yml": "", Makefile: "", [SCHEME]: null }));
  const app = errors.filter((e) => !e.includes("utils/swift"));
  assert.equal(app.length, 4, errors.join("\n"));
});

test("an Xcode project outside IOS_APPS fails", () => {
  const errors = checkIos(...fake({ "recap/ios/Recap.xcodeproj/project.pbxproj": GOOD_PBX }));
  assert.match(errors[0], /^recap\/ios\/Recap\.xcodeproj\/project\.pbxproj: Xcode project outside IOS_APPS/);
});

test("a per-app swift-format config fails", () => {
  assert.match(checkIos(...fake({ "scout/ios/.swift-format": "{}" }))[0], /per-app Swift config/);
});

test("a Swift package outside SWIFT_PACKAGES fails", () => {
  const errors = checkIos(...fake({ "scout/ios/Kit/Package.swift": "" }));
  assert.match(errors[0], /^scout\/ios\/Kit\/Package\.swift: Swift package outside SWIFT_PACKAGES/);
});

test("a shared Swift package without its gate fails", () => {
  const ci = GOOD_CI.replace("run: ./scripts/ci/ios.sh utils/swift\n", "");
  const errors = checkIos(...fake({ ".github/workflows/ci.yml": ci }));
  assert.deepEqual(errors, [".github/workflows/ci.yml: no job runs `scripts/ci/ios.sh utils/swift`"]);
});
