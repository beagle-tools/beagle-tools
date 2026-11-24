// Unit tests for the test-selection rules. Run: make test-scripts
import assert from "node:assert/strict";
import { describe, test } from "node:test";
import {
  fullWorkspaceReason,
  includeTargets,
  isUnder,
  ownersOf,
  selectPackages,
} from "./affected-lib.mjs";

const node = (name, dir, deps = [], extra = []) => [dir, { name, dir, deps, extra }];

// Mirrors the real rca + rca/desktop/src-tauri layout.
const rca = {
  members: new Set(["rca/tui", "rca/lib/agent", "rca/daemon"]),
  nodes: new Map([
    node("beagle", "rca/tui", ["utils"], ["skills/beagle/SKILL.md"]),
    node("beagle-agent", "rca/lib/agent", ["rca/tui", "utils"]),
    node("beagle-agentd", "rca/daemon", ["rca/tui", "rca/lib/agent", "utils"]),
    node("beagle-utils", "utils"),
  ]),
};
const desktop = {
  members: new Set(["rca/desktop/src-tauri"]),
  nodes: new Map([
    node("beagle-desktop", "rca/desktop/src-tauri", ["rca/tui"]),
    node("beagle", "rca/tui", ["utils"], ["rca/Cargo.toml"]),
    node("beagle-utils", "utils"),
  ]),
};
const names = (result) => {
  assert.equal(result.all, false);
  return result.packages.map((p) => p.name);
};

describe("isUnder", () => {
  test("matches the path itself and its children only", () => {
    assert.ok(isUnder("rca/tui", "rca/tui"));
    assert.ok(isUnder("rca/tui/src/main.rs", "rca/tui"));
    assert.ok(!isUnder("rca/tui-extra/x.rs", "rca/tui"));
  });
});

describe("fullWorkspaceReason", () => {
  for (const file of [
    "utils/src/lib.rs",
    "rust-toolchain.toml",
    "scripts/ci/rust.sh",
    "standards/cargo-lints.toml",
    ".github/workflows/_rust.yml",
    "rca/Cargo.toml",
    "rca/Cargo.lock",
    "rca/.cargo/config.toml",
  ]) {
    test(`${file} forces the whole rca workspace`, () => {
      assert.notEqual(fullWorkspaceReason([file], "rca"), null);
    });
  }

  test("another workspace's lockfile does not", () => {
    assert.equal(fullWorkspaceReason(["tandem/Cargo.lock"], "rca"), null);
  });

  test("a package manifest is not the workspace manifest", () => {
    assert.equal(fullWorkspaceReason(["rca/tui/Cargo.toml"], "rca"), null);
  });

  test("the repo-root workspace (`.`) uses the root manifest and lockfile", () => {
    for (const file of ["Cargo.toml", "Cargo.lock", ".cargo/config.toml"]) {
      assert.notEqual(fullWorkspaceReason([file], "."), null, file);
    }
    assert.equal(fullWorkspaceReason(["rca/tui/Cargo.toml"], "."), null);
    assert.equal(fullWorkspaceReason(["tandem/desktop/src-tauri/Cargo.lock"], "."), null);
  });

  test("release workflows do not force a full run", () => {
    assert.equal(fullWorkspaceReason([".github/workflows/release.yml"], "rca"), null);
  });
});

describe("ownersOf", () => {
  test("the most specific package dir wins", () => {
    const nodes = new Map([node("outer", "a"), node("inner", "a/b")]);
    assert.deepEqual([...ownersOf("a/b/src/lib.rs", nodes)], ["a/b"]);
    assert.deepEqual([...ownersOf("a/src/lib.rs", nodes)], ["a"]);
  });

  test("compiled-in files outside the package dir belong to it", () => {
    assert.deepEqual([...ownersOf("skills/beagle/SKILL.md", rca.nodes)], ["rca/tui"]);
  });

  test("files outside every package have no owner", () => {
    assert.equal(ownersOf("rca/web/src/index.astro", rca.nodes).size, 0);
  });
});

describe("selectPackages", () => {
  test("a leaf change tests only that package", () => {
    const result = selectPackages({
      changed: ["rca/daemon/src/main.rs"],
      ws: "rca",
      graph: rca,
    });
    assert.deepEqual(names(result), ["beagle-agentd"]);
    assert.match(result.packages[0].reason, /^changed: /);
  });

  test("a shared crate change pulls in its reverse dependents transitively", () => {
    const result = selectPackages({ changed: ["rca/tui/src/lib.rs"], ws: "rca", graph: rca });
    assert.deepEqual(names(result), ["beagle", "beagle-agent", "beagle-agentd"]);
  });

  test("a mid-graph change skips its dependencies", () => {
    const result = selectPackages({
      changed: ["rca/lib/agent/src/lib.rs"],
      ws: "rca",
      graph: rca,
    });
    assert.deepEqual(names(result), ["beagle-agent", "beagle-agentd"]);
  });

  test("a path dep in another workspace re-tests its users here", () => {
    const result = selectPackages({ changed: ["rca/tui/src/lib.rs"], ws: "rca/desktop/src-tauri", graph: desktop });
    assert.deepEqual(names(result), ["beagle-desktop"]);
    assert.equal(result.packages[0].reason, "depends on beagle");
  });

  test("an external dep's workspace manifest counts as its input", () => {
    const result = selectPackages({ changed: ["rca/Cargo.toml"], ws: "rca/desktop/src-tauri", graph: desktop });
    assert.deepEqual(names(result), ["beagle-desktop"]);
  });

  test("an embedded skill file re-tests the crate that includes it", () => {
    const result = selectPackages({ changed: ["skills/beagle/SKILL.md"], ws: "rca", graph: rca });
    assert.deepEqual(names(result), ["beagle", "beagle-agent", "beagle-agentd"]);
  });

  test("unrelated changes select nothing", () => {
    const result = selectPackages({
      changed: ["rca/web/src/pages/index.astro", "tandem/desktop/src/App.tsx", "README.md"],
      ws: "rca",
      graph: rca,
    });
    assert.deepEqual(names(result), []);
  });

  test("a global trigger selects everything regardless of other files", () => {
    const result = selectPackages({
      changed: ["rca/tui/src/lib.rs", "utils/src/fs.rs"],
      ws: "rca",
      graph: rca,
    });
    assert.equal(result.all, true);
    assert.match(result.reason, /utils\/src\/fs\.rs/);
  });

  test("an empty change selects nothing", () => {
    assert.deepEqual(names(selectPackages({ changed: [], ws: "rca", graph: rca })), []);
  });
});

describe("includeTargets", () => {
  test("finds str and bytes includes, ignores other macros", () => {
    const src = `
      const A: &str = include_str!("../../../skills/beagle/SKILL.md");
      const B: &[u8] = include_bytes!( "../assets/icon.png" );
      const C: &str = env!("CARGO_PKG_VERSION");
    `;
    assert.deepEqual(includeTargets(src), ["../../../skills/beagle/SKILL.md", "../assets/icon.png"]);
  });
});
