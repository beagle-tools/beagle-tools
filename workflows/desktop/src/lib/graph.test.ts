import { describe, expect, it } from "vitest";

import { NODE_H, NODE_W, chains, edgePath, layout } from "./graph";
import { makeRow } from "./testing";

describe("layout", () => {
  it("places a lone root at the origin with a one-node canvas", () => {
    const g = layout([makeRow("a", 0)]);
    expect(g.nodes).toHaveLength(1);
    expect(g.nodes[0]).toMatchObject({ x: 0, y: 0 });
    expect(g.edges).toHaveLength(0);
    expect(g).toMatchObject({ width: NODE_W, height: NODE_H });
  });

  it("puts depth in columns and centres a parent on its children", () => {
    const rows = [makeRow("p", 0, ["c1", "c2"]), makeRow("c1", 1), makeRow("c2", 1)];
    const g = layout(rows);
    const at = (id: string) => g.nodes.find((n) => n.row.run.id === id);
    const [p, c1, c2] = [at("p"), at("c1"), at("c2")];
    expect(c1?.x).toBeGreaterThan(0);
    expect(c1?.x).toBe(c2?.x);
    expect(c2?.y).toBeGreaterThan(c1?.y ?? Infinity);
    expect(p?.x).toBe(0);
    expect(p?.y).toBe(((c1?.y ?? 0) + (c2?.y ?? 0)) / 2);
    expect(g.edges.map((e) => [e.from.row.run.id, e.to.row.run.id])).toEqual([
      ["p", "c1"],
      ["p", "c2"],
    ]);
  });

  it("still draws a row whose parent is outside the slice", () => {
    const g = layout([makeRow("orphan", 2)]);
    expect(g.nodes.map((n) => n.row.run.id)).toEqual(["orphan"]);
  });

  it("ignores child ids that are not in the slice", () => {
    const g = layout([makeRow("p", 0, ["gone"])]);
    expect(g.nodes).toHaveLength(1);
    expect(g.edges).toHaveLength(0);
  });
});

describe("edgePath", () => {
  it("runs from the parent's right edge to the child's left edge", () => {
    const rows = [makeRow("p", 0, ["c"]), makeRow("c", 1)];
    const [edge] = layout(rows).edges;
    expect(edge).toBeDefined();
    if (!edge) return;
    const d = edgePath(edge);
    expect(d.startsWith(`M ${String(NODE_W)} ${String(NODE_H / 2)} C `)).toBe(true);
    expect(d.endsWith(`${String(edge.to.x)} ${String(edge.to.y + NODE_H / 2)}`)).toBe(true);
  });
});

describe("chains", () => {
  it("starts a new chain at every root", () => {
    const rows = [makeRow("a", 0), makeRow("a1", 1), makeRow("b", 0), makeRow("b1", 1)];
    expect(chains(rows).map((c) => c.map((r) => r.run.id))).toEqual([
      ["a", "a1"],
      ["b", "b1"],
    ]);
  });

  it("opens a chain for a leading non-root row instead of dropping it", () => {
    expect(chains([makeRow("x", 1), makeRow("y", 2)]).map((c) => c.length)).toEqual([2]);
  });

  it("is empty for no rows", () => {
    expect(chains([])).toEqual([]);
  });
});
