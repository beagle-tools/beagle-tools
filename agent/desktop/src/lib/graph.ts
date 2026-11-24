// Layout for the chain graph: depth becomes a column, and a parent sits
// centred on its children — the "tidy tree" arrangement GitHub Actions
// uses for a workflow. Pure functions, no React, so the geometry can be
// reasoned about (and tested) without rendering anything.

import type { ForestRow } from "./types";

export const NODE_W = 204;
export const NODE_H = 66;
const COL_GAP = 68;
const ROW_GAP = 16;

export interface GraphNode {
  row: ForestRow;
  x: number;
  y: number;
}

export interface GraphEdge {
  from: GraphNode;
  to: GraphNode;
}

export interface Graph {
  nodes: GraphNode[];
  edges: GraphEdge[];
  width: number;
  height: number;
}

export function layout(rows: ForestRow[]): Graph {
  const byId = new Map(rows.map((r) => [r.run.id, r]));
  const placed = new Map<string, GraphNode>();

  // Leaves consume slots top to bottom; a parent then centres on the span
  // its children occupy. Walking depth-first means every child is placed
  // before the parent that needs their positions.
  let nextSlot = 0;
  const place = (row: ForestRow): GraphNode => {
    const existing = placed.get(row.run.id);
    if (existing) return existing;

    const kids = row.children.map((id) => byId.get(id)).filter((r): r is ForestRow => !!r);
    let slot: number;
    if (kids.length === 0) {
      slot = nextSlot++;
    } else {
      const kidNodes = kids.map(place);
      const firstY = kidNodes[0]?.y ?? 0;
      const lastY = kidNodes.at(-1)?.y ?? firstY;
      slot = (firstY + lastY) / 2 / (NODE_H + ROW_GAP);
    }
    const node: GraphNode = {
      row,
      x: row.depth * (NODE_W + COL_GAP),
      y: slot * (NODE_H + ROW_GAP),
    };
    placed.set(row.run.id, node);
    return node;
  };

  // Roots in the order given (newest chain first).
  for (const row of rows) if (row.depth === 0) place(row);
  // A row whose parent is missing from this slice would otherwise never
  // be placed — draw it rather than silently dropping it.
  for (const row of rows) place(row);

  const nodes = [...placed.values()];
  const edges: GraphEdge[] = [];
  for (const node of nodes) {
    for (const childId of node.row.children) {
      const child = placed.get(childId);
      if (child) edges.push({ from: node, to: child });
    }
  }

  return {
    nodes,
    edges,
    width: Math.max(...nodes.map((n) => n.x + NODE_W), NODE_W),
    height: Math.max(...nodes.map((n) => n.y + NODE_H), NODE_H),
  };
}

/** An S-curve from a parent's right edge to a child's left edge. */
export function edgePath(edge: GraphEdge): string {
  const x1 = edge.from.x + NODE_W;
  const y1 = edge.from.y + NODE_H / 2;
  const x2 = edge.to.x;
  const y2 = edge.to.y + NODE_H / 2;
  const bend = Math.max(24, (x2 - x1) / 2);
  const pt = (x: number, y: number): string => `${String(x)} ${String(y)}`;
  return `M ${pt(x1, y1)} C ${pt(x1 + bend, y1)}, ${pt(x2 - bend, y2)}, ${pt(x2, y2)}`;
}

/** One chain's rows in depth-first order; the root is always first. */
export type Chain = [ForestRow, ...ForestRow[]];

/**
 * Split the flat forest into one list per chain.
 *
 * Every chain then draws in its own card. Drawing them all into one SVG
 * made eighteen unrelated chains read as a single tangled graph: nothing
 * said where one piece of work ended and the next began, which is the
 * thing the flows list gets right by indenting.
 */
export function chains(rows: ForestRow[]): Chain[] {
  const out: Chain[] = [];
  for (const row of rows) {
    const current = out.at(-1);
    if (row.depth === 0 || current === undefined) out.push([row]);
    else current.push(row);
  }
  return out;
}
