// The iPhone apps as a check: every Xcode project is a known app with a
// gate, shares one Swift format config, and commits no signing identity.
// Pure (no fs), so scripts/ci/ios-standards.test.mjs can feed it any tree.

/**
 * The iPhone apps, each a folder holding one `<Name>.xcodeproj` with a shared
 * `<Name>` scheme: today the one Beagle app at `ios/` (docs/LAYOUT.md). Adding
 * one means adding it here, to .github/workflows/ci.yml and to the root
 * Makefile (AGENTS.md).
 */
export const IOS_APPS = ["ios"];

/**
 * The Swift packages the apps share (like the Rust `utils/` crates), each a
 * folder with a Package.swift, gated by the same script on a simulator.
 */
export const SWIFT_PACKAGES = ["utils/swift"];

/**
 * Checks the iPhone apps. `read(path)` returns a repo file's text, `files`
 * lists every repo file (repo-relative). Returns the violations.
 */
export function checkIos(read, files) {
  const errors = [];
  const ci = read(".github/workflows/ci.yml");
  const makefile = read("Makefile");

  for (const f of files) {
    const base = f.split("/").pop();
    if ([".swift-format", ".swiftformat", ".swiftlint.yml"].includes(base)) {
      errors.push(`${f}: per-app Swift config is not allowed; edit standards/swift/swift-format.json`);
    }
    if (base === "Package.swift" && !SWIFT_PACKAGES.includes(f.slice(0, -"/Package.swift".length))) {
      errors.push(`${f}: Swift package outside SWIFT_PACKAGES (scripts/ci/ios-standards.mjs) — shared Swift code goes in utils/swift`);
    }
    const project = f.match(/^(.*)\/([^/]+)\.xcodeproj\/project\.pbxproj$/);
    if (project && !IOS_APPS.includes(project[1])) {
      errors.push(`${f}: Xcode project outside IOS_APPS (scripts/ci/ios-standards.mjs) — it would have no gate`);
    }
  }

  for (const app of IOS_APPS) {
    const pbx = files.filter((f) => f.startsWith(`${app}/`) && /^[^/]+\.xcodeproj\/project\.pbxproj$/.test(f.slice(app.length + 1)));
    if (pbx.length !== 1) {
      errors.push(`${app}: must hold exactly one .xcodeproj (found ${pbx.length})`);
      continue;
    }
    const name = pbx[0].slice(app.length + 1).split(".xcodeproj")[0];
    const scheme = `${app}/${name}.xcodeproj/xcshareddata/xcschemes/${name}.xcscheme`;
    if (!files.includes(scheme)) errors.push(`${scheme}: missing — scripts/ci/ios.sh tests the shared scheme`);

    const text = read(pbx[0]);
    // Signing is the developer's own: Xcode picks the team locally, never the repo.
    for (const m of text.matchAll(/DEVELOPMENT_TEAM\s*=\s*"?([^";]*)"?;/g)) {
      if (m[1].trim()) errors.push(`${pbx[0]}: DEVELOPMENT_TEAM is set — never commit a team ID`);
    }
    if (/PROVISIONING_PROFILE(_SPECIFIER)?\s*=\s*"?[^";\s]/.test(text)) {
      errors.push(`${pbx[0]}: a provisioning profile is committed — signing must stay automatic`);
    }
    if (/CODE_SIGN_STYLE\s*=\s*Manual/.test(text)) {
      errors.push(`${pbx[0]}: CODE_SIGN_STYLE = Manual — signing must stay automatic`);
    }

    const gate = `scripts/ci/ios.sh ${app}`;
    if (!ci.includes(gate)) errors.push(`.github/workflows/ci.yml: no job runs \`${gate}\``);
    if (!ci.includes(`"${app}/**"`)) errors.push(`.github/workflows/ci.yml: no path filter "${app}/**"`);
    if (!makefile.includes(gate)) errors.push(`Makefile: no target runs \`${gate}\``);
  }
  for (const pkg of SWIFT_PACKAGES) {
    if (!files.includes(`${pkg}/Package.swift`)) errors.push(`${pkg}/Package.swift: missing (listed in SWIFT_PACKAGES)`);
    const gate = `scripts/ci/ios.sh ${pkg}`;
    if (!ci.includes(gate)) errors.push(`.github/workflows/ci.yml: no job runs \`${gate}\``);
    if (!ci.includes(`"${pkg}/**"`)) errors.push(`.github/workflows/ci.yml: no path filter "${pkg}/**"`);
    if (!makefile.includes(gate)) errors.push(`Makefile: no target runs \`${gate}\``);
  }
  return errors;
}
