// Pure selection logic behind scripts/ci/affected.mjs: given the files a
// change touched and a cargo workspace's package graph, decide which
// workspace members must be tested. No git, no cargo, no filesystem here, so
// scripts/ci/affected.test.mjs can exercise every rule directly.
//
// Graph shape (built by affected.mjs from `cargo metadata`):
//   { members: Set<dir>, nodes: Map<dir, { name, dir, deps: dir[], extra: path[] }> }
// Every dir/path is repo-relative with forward slashes and no trailing slash.
// `nodes` holds the workspace members AND every path dependency they reach
// outside the workspace (e.g. rca/desktop/src-tauri -> rca/cli -> utils),
// because a change to one of those must re-test the members that use it.
// `extra` lists inputs outside the package dir that the package compiles in
// (include_str!/include_bytes! targets, an external dep's workspace manifest).

/** Paths that re-test every workspace: shared compiler, lints, tooling, CI. */
export const GLOBAL_TRIGGERS = [
  "utils",
  "rust-toolchain.toml",
  "scripts",
  "standards",
  ".github/workflows/ci.yml",
  ".github/workflows/_rust.yml",
];

/** True when `file` is `path` or lives under directory `path`. */
export function isUnder(file, path) {
  return path === "" || file === path || file.startsWith(`${path}/`);
}

/**
 * Returns why the whole workspace must be tested, or null when per-package
 * selection is safe.
 * @param {string[]} changed repo-relative changed files
 * @param {string} ws repo-relative workspace dir (where Cargo.lock lives)
 */
export function fullWorkspaceReason(changed, ws) {
  const local = [`${ws}/Cargo.toml`, `${ws}/Cargo.lock`, `${ws}/.cargo`];
  for (const file of changed) {
    const hit = GLOBAL_TRIGGERS.find((t) => isUnder(file, t));
    if (hit) return `${file} changed (${hit} affects every workspace)`;
    if (local.some((t) => isUnder(file, t))) {
      return `${file} changed (workspace manifest/lockfile/config)`;
    }
  }
  return null;
}

/**
 * The graph nodes a changed file belongs to: the package whose directory
 * most specifically contains it, plus any package that compiles the file in
 * from outside its own directory.
 */
export function ownersOf(file, nodes) {
  const owners = new Set();
  let best = null;
  for (const node of nodes.values()) {
    if (isUnder(file, node.dir) && (best === null || node.dir.length > best.dir.length)) {
      best = node;
    }
    if (node.extra.some((p) => isUnder(file, p))) owners.add(node.dir);
  }
  if (best) owners.add(best.dir);
  return owners;
}

/**
 * Selects the workspace members to test.
 * @returns {{ all: true, reason: string } | { all: false, packages: { name: string, reason: string }[] }}
 *   `packages` is sorted by name and empty when nothing in this workspace is affected.
 */
export function selectPackages({ changed, ws, graph }) {
  const reason = fullWorkspaceReason(changed, ws);
  if (reason) return { all: true, reason };

  // why.get(dir) = the human-readable cause that first reached that node.
  const why = new Map();
  for (const file of changed) {
    for (const dir of ownersOf(file, graph.nodes)) {
      if (!why.has(dir)) why.set(dir, `changed: ${file}`);
    }
  }

  // Reverse-dependency closure: anything that (transitively) depends on an
  // affected node is affected too.
  const dependents = new Map();
  for (const node of graph.nodes.values()) {
    for (const dep of node.deps) {
      if (!dependents.has(dep)) dependents.set(dep, []);
      dependents.get(dep).push(node.dir);
    }
  }
  const queue = [...why.keys()];
  while (queue.length > 0) {
    const dir = queue.shift();
    const name = graph.nodes.get(dir)?.name ?? dir;
    for (const user of dependents.get(dir) ?? []) {
      if (why.has(user)) continue;
      why.set(user, `depends on ${name}`);
      queue.push(user);
    }
  }

  const packages = [...why.entries()]
    .filter(([dir]) => graph.members.has(dir))
    .map(([dir, cause]) => ({ name: graph.nodes.get(dir).name, reason: cause }))
    .sort((a, b) => a.name.localeCompare(b.name));
  return { all: false, packages };
}

/** Collects relative include_str!/include_bytes! targets from Rust source. */
export function includeTargets(source) {
  const out = [];
  const re = /include_(?:str|bytes)!\s*\(\s*"([^"]+)"\s*\)/g;
  for (const match of source.matchAll(re)) out.push(match[1]);
  return out;
}
