// Pure release-planning logic (no git, no network) so it can be unit tested.
// plan.mjs wires it to git/gh. The rules are the release contract in
// docs/RELEASING.md — change both together.

/**
 * Apps that release, and where each one's first version comes from. `dir`
 * is the app's directory when it differs from the app name.
 */
export const APPS = {
  tandem: { versionFile: "tandem/src-tauri/tauri.conf.json", kind: "json" },
  rca: { versionFile: "rca/cli/Cargo.toml", kind: "cargo-package" },
  workflows: { versionFile: "workflows/Cargo.toml", kind: "cargo-workspace" },
  cli: { dir: "cli-base", versionFile: "cli-base/Cargo.toml", kind: "cargo-workspace" },
};

/** Paths compiled into every app: a change here releases all of them. */
const SHARED_PREFIXES = ["utils/"];
const SHARED_FILES = new Set(["rust-toolchain.toml"]);
/**
 * Paths outside `<app>/` compiled into one app. Checked before the `.md`
 * exclusion: rca embeds these SKILL.md files with include_str!.
 */
const EXTRA_PREFIXES = { rca: ["skills/beagle/", "skills/beagle-review/"] };

const ZERO_SHA = /^0+$/;
const SEMVER = /^(\d+)\.(\d+)\.(\d+)$/;

/** True when `before` can't be diffed against (first push, force push). */
export function isUnknownBefore(before) {
  return !before || ZERO_SHA.test(before);
}

/**
 * Whether a changed path should release `app`. Docs-only changes never do:
 * `**\/*.md` and `<dir>/docs/**` are excluded everywhere, except for files
 * an app embeds (EXTRA_PREFIXES).
 */
export function isReleaseRelevant(app, path) {
  if ((EXTRA_PREFIXES[app] ?? []).some((p) => path.startsWith(p))) return true;
  if (path.endsWith(".md")) return false;
  if (SHARED_FILES.has(path)) return true;
  if (SHARED_PREFIXES.some((p) => path.startsWith(p))) return true;
  const dir = APPS[app]?.dir ?? app;
  if (!path.startsWith(`${dir}/`)) return false;
  return !path.startsWith(`${dir}/docs/`);
}

/** The apps any of `paths` releases, in APPS order. */
export function changedApps(paths) {
  return Object.keys(APPS).filter((app) => paths.some((p) => isReleaseRelevant(app, p)));
}

export function parseSemver(version) {
  const m = SEMVER.exec(version);
  if (!m) return null;
  return m.slice(1, 4).map(Number);
}

export function compareSemver(a, b) {
  const pa = parseSemver(a);
  const pb = parseSemver(b);
  for (let i = 0; i < 3; i += 1) {
    if (pa[i] !== pb[i]) return pa[i] - pb[i];
  }
  return 0;
}

/** `release:major` > `release:minor` > patch (the default). */
export function bumpFromLabels(labels) {
  if (labels.includes("release:major")) return "major";
  if (labels.includes("release:minor")) return "minor";
  return "patch";
}

export function bumpVersion(version, bump) {
  const parsed = parseSemver(version);
  if (!parsed) throw new Error(`not a semver version: "${version}"`);
  const [maj, min, pat] = parsed;
  if (bump === "major") return `${maj + 1}.0.0`;
  if (bump === "minor") return `${maj}.${min + 1}.0`;
  if (bump === "patch") return `${maj}.${min}.${pat + 1}`;
  throw new Error(`unknown bump "${bump}"`);
}

export const tagFor = (app, version) => `beagle-${app}-v${version}`;

/** Versions from `beagle-<app>-vX.Y.Z` tags, highest first. Others ignored. */
export function appVersions(app, tags) {
  const prefix = `beagle-${app}-v`;
  return tags
    .filter((t) => t.startsWith(prefix))
    .map((t) => t.slice(prefix.length))
    .filter((v) => parseSemver(v))
    .sort((a, b) => compareSemver(b, a));
}

/** Reads the app's version out of its version file's text. */
export function readVersion(kind, text) {
  if (kind === "json") return JSON.parse(text).version;
  const section = kind === "cargo-workspace" ? "workspace.package" : "package";
  let inSection = false;
  for (const raw of text.split("\n")) {
    const line = raw.trim();
    if (line.startsWith("[")) {
      inSection = line === `[${section}]`;
      continue;
    }
    const m = inSection && /^version\s*=\s*"([^"]+)"/.exec(line);
    if (m) return m[1];
  }
  throw new Error(`no version in [${section}]`);
}

/**
 * Decides one app's release. Order matters:
 *  1. HEAD already tagged for the app -> release that version as-is (re-runs
 *     and manual re-releases), even when nothing changed.
 *  2. No release-relevant change -> skip.
 *  3. main moved past this commit -> skip; the newer push's run releases.
 *  4. No tag for the app yet -> the version file's value.
 *  5. Otherwise bump the highest tag by the merged PR's labels.
 */
export function planApp({ app, changed, headTags, allTags, labels, stale, fileVersion }) {
  const skip = (reason) => ({ app, release: false, action: "skip", reason });
  const existing = appVersions(app, headTags)[0];
  if (existing) {
    return {
      app,
      release: true,
      action: "reuse",
      version: existing,
      tag: tagFor(app, existing),
      reason: "HEAD already tagged",
    };
  }
  if (!changed) return skip("no release-relevant changes");
  if (stale) return skip("main moved past this commit; the newer run releases");
  const latest = appVersions(app, allTags)[0];
  if (!latest) {
    if (!parseSemver(fileVersion ?? "")) {
      throw new Error(`${app}: version file holds "${fileVersion}", not X.Y.Z`);
    }
    return {
      app,
      release: true,
      action: "first",
      version: fileVersion,
      tag: tagFor(app, fileVersion),
      reason: "no tags yet; using the version file",
    };
  }
  const bump = bumpFromLabels(labels);
  const version = bumpVersion(latest, bump);
  return {
    app,
    release: true,
    action: "bump",
    version,
    tag: tagFor(app, version),
    reason: `${bump} bump from ${latest}`,
  };
}

/** `KEY=value` lines for $GITHUB_OUTPUT, one set per app plus `any`. */
export function toOutputs(plans) {
  const lines = [];
  for (const p of plans) {
    lines.push(`${p.app}=${p.release}`);
    lines.push(`${p.app}_version=${p.version ?? ""}`);
    lines.push(`${p.app}_tag=${p.tag ?? ""}`);
  }
  lines.push(`any=${plans.some((p) => p.release)}`);
  return lines;
}
