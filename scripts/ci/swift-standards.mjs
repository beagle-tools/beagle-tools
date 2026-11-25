// The shared Swift settings as a check (the Swift side of the cargo-lints
// drift check): every Xcode project builds on standards/swift/Shared.xcconfig
// and overrides none of it, every Swift package applies the shared
// swiftSettings to every target, and dependencies are locked.
// Pure (no fs), so scripts/ci/swift-standards.test.mjs can feed it any tree.
import { IOS_APPS, SWIFT_PACKAGES } from "./ios-standards.mjs";

const XCCONFIG = "standards/swift/Shared.xcconfig";
const PACKAGE_SETTINGS = "standards/swift/package-settings.txt";

/** The keys an xcconfig sets. */
export function xcconfigKeys(text) {
  return [...text.matchAll(/^\s*([A-Z_][A-Z0-9_]*)\s*=/gm)].map((m) => m[1]);
}

/** `{ id: body }` for every `id /* … *\/ = { … };` object of one isa in a pbxproj. */
function objects(pbx, isa) {
  const out = {};
  const re = /^\t\t([0-9A-F]{24}) \/\*[^*]*\*\/ = \{\n([\s\S]*?)^\t\t\};/gm;
  for (const m of pbx.matchAll(re)) {
    if (m[2].includes(`isa = ${isa};`)) out[m[1]] = m[2];
  }
  return out;
}

function checkProject(path, pbx, sharedKeys, files, errors) {
  const app = path.split("/").slice(0, -2).join("/");
  const depth = app.split("/").length;
  const relative = `${"../".repeat(depth)}${XCCONFIG}`;
  const ref = pbx.match(new RegExp(`\\t\\t([0-9A-F]{24}) /\\*[^*]*\\*/ = \\{isa = PBXFileReference;[^}]*path = ${relative.replace(/[.]/g, "\\.")};`));
  if (!ref) {
    errors.push(`${path}: no file reference to ${relative} — every project builds on the shared xcconfig`);
    return;
  }
  const project = Object.values(objects(pbx, "PBXProject"))[0] ?? "";
  const listId = project.match(/buildConfigurationList = ([0-9A-F]{24})/)?.[1];
  const list = listId ? objects(pbx, "XCConfigurationList")[listId] ?? "" : "";
  const configs = objects(pbx, "XCBuildConfiguration");
  for (const id of [...list.matchAll(/([0-9A-F]{24}) \/\*/g)].map((m) => m[1])) {
    if (!configs[id]?.includes(`baseConfigurationReference = ${ref[1]} `)) {
      errors.push(`${path}: project configuration ${id} is not based on ${XCCONFIG}`);
    }
  }
  for (const [id, body] of Object.entries(configs)) {
    for (const key of sharedKeys) {
      if (new RegExp(`^\\t\\t\\t\\t${key} = `, "m").test(body)) {
        errors.push(`${path}: configuration ${id} sets ${key}, which ${XCCONFIG} owns — don't override shared settings`);
      }
    }
  }
  if (/isa = XCRemoteSwiftPackageReference;/.test(pbx)) {
    const resolved = `${path.slice(0, -"project.pbxproj".length)}project.xcworkspace/xcshareddata/swiftpm/Package.resolved`;
    if (!files.includes(resolved)) errors.push(`${resolved}: missing — commit it with any package dependency`);
  }
}

function checkPackage(dir, text, setting, files, errors) {
  const path = `${dir}/Package.swift`;
  const tools = text.match(/^\/\/ swift-tools-version:\s*(\d+)\.(\d+)/);
  if (!tools || Number(tools[1]) * 100 + Number(tools[2]) < 602) {
    errors.push(`${path}: swift-tools-version must be 6.2 or newer (treatAllWarnings)`);
  }
  if (!text.includes(setting)) errors.push(`${path}: must define \`${setting}\` (${PACKAGE_SETTINGS})`);
  const targets = (text.match(/\.(?:target|testTarget|executableTarget)\(/g) ?? []).length;
  const strict = (text.match(/swiftSettings: strict\b/g) ?? []).length;
  if (targets !== strict) {
    errors.push(`${path}: ${targets} targets but ${strict} use \`swiftSettings: strict\` — every target must`);
  }
  if (/\.package\(/.test(text) && !files.includes(`${dir}/Package.resolved`)) {
    errors.push(`${dir}/Package.resolved: missing — commit it with any package dependency`);
  }
}

/** Checks every app and package against the shared Swift settings. */
export function checkSwiftStandards(read, files) {
  const errors = [];
  const sharedKeys = xcconfigKeys(read(XCCONFIG));
  if (sharedKeys.length === 0) errors.push(`${XCCONFIG}: sets nothing`);
  const setting = read(PACKAGE_SETTINGS).trim();
  for (const app of IOS_APPS) {
    for (const f of files.filter((p) => p.startsWith(`${app}/`) && p.endsWith(".xcodeproj/project.pbxproj"))) {
      checkProject(f, read(f), sharedKeys, files, errors);
    }
  }
  for (const pkg of SWIFT_PACKAGES) {
    if (files.includes(`${pkg}/Package.swift`)) checkPackage(pkg, read(`${pkg}/Package.swift`), setting, files, errors);
  }
  for (const f of files.filter((p) => p.endsWith(".xcconfig") && p !== XCCONFIG)) {
    errors.push(`${f}: per-project xcconfig is not allowed; settings live in ${XCCONFIG} or the project`);
  }
  return errors;
}
