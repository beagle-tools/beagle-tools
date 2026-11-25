import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

import { checkSwiftStandards, xcconfigKeys } from "./swift-standards.mjs";

const root = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const real = (p) => readFileSync(join(root, p), "utf8");

const PBX = "ios/Beagle.xcodeproj/project.pbxproj";
const PKG = "utils/swift/Package.swift";
const FILES = ["standards/swift/Shared.xcconfig", "standards/swift/package-settings.txt", PBX, PKG];

function tree(overrides = {}, extraFiles = []) {
  const read = (p) => (p in overrides ? overrides[p] : real(p));
  return [read, [...FILES, ...extraFiles]];
}

test("the repo's own project and package pass", () => {
  assert.deepEqual(checkSwiftStandards(...tree()), []);
});

test("the shared xcconfig's keys are read", () => {
  assert.ok(xcconfigKeys(real("standards/swift/Shared.xcconfig")).includes("SWIFT_TREAT_WARNINGS_AS_ERRORS"));
});

test("a target that overrides a shared setting fails", () => {
  const pbx = real(PBX).replace(
    "\t\t\t\tPRODUCT_NAME = \"$(TARGET_NAME)\";",
    "\t\t\t\tPRODUCT_NAME = \"$(TARGET_NAME)\";\n\t\t\t\tSWIFT_TREAT_WARNINGS_AS_ERRORS = NO;",
  );
  const errors = checkSwiftStandards(...tree({ [PBX]: pbx }));
  assert.equal(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /sets SWIFT_TREAT_WARNINGS_AS_ERRORS, which standards\/swift\/Shared\.xcconfig owns/);
});

test("a project not based on the shared xcconfig fails", () => {
  const pbx = real(PBX).replaceAll(/\t\t\tbaseConfigurationReference = [0-9A-F]{24} \/\* Shared\.xcconfig \*\/;\n/g, "");
  const errors = checkSwiftStandards(...tree({ [PBX]: pbx }));
  assert.equal(errors.length, 2, errors.join("\n"));
  assert.match(errors[0], /is not based on standards\/swift\/Shared\.xcconfig/);
});

test("a package target without the shared settings fails", () => {
  const pkg = real(PKG).replace('.target(name: "BeagleAPI", swiftSettings: strict)', '.target(name: "BeagleAPI")');
  const errors = checkSwiftStandards(...tree({ [PKG]: pkg }));
  assert.equal(errors.length, 1, errors.join("\n"));
  assert.match(errors[0], /use `swiftSettings: strict`/);
});

test("a package that weakens the shared settings fails", () => {
  const pkg = real(PKG).replace("[.swiftLanguageMode(.v6)]", "[]");
  assert.match(checkSwiftStandards(...tree({ [PKG]: pkg }))[0], /must define `let strict/);
});

test("treatAllWarnings in a package fails (it breaks apps that depend on it)", () => {
  const pkg = real(PKG).replace("[.swiftLanguageMode(.v6)]", "[.swiftLanguageMode(.v6), .treatAllWarnings(as: .error)]");
  const errors = checkSwiftStandards(...tree({ [PKG]: pkg }));
  assert.ok(errors.some((e) => /`treatAllWarnings` breaks builds/.test(e)), errors.join("\n"));
});

test("a package gate without warnings-as-errors fails", () => {
  const gate = real("scripts/ci/ios.sh").replaceAll("SWIFT_TREAT_WARNINGS_AS_ERRORS=YES", "");
  const errors = checkSwiftStandards(...tree({ "scripts/ci/ios.sh": gate }));
  assert.match(errors[0], /must build Swift packages with SWIFT_TREAT_WARNINGS_AS_ERRORS=YES/);
});

test("a dependency without Package.resolved fails", () => {
  const pkg = real(PKG).replace(
    "  products: [",
    '  dependencies: [.package(url: "https://example.com/x.git", from: "1.0.0")],\n  products: [',
  );
  assert.match(checkSwiftStandards(...tree({ [PKG]: pkg }))[0], /^utils\/swift\/Package\.resolved: missing/);
});

test("a per-project xcconfig fails", () => {
  const errors = checkSwiftStandards(...tree({}, ["ios/Local.xcconfig"]));
  assert.match(errors[0], /^ios\/Local\.xcconfig: per-project xcconfig is not allowed/);
});
