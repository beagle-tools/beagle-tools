// Box-drawing guides for rows in depth-first order — a port of agenttop's
// `tree_prefixes`, so the desktop draws a job's tree exactly the way the
// terminal does. Row 0 is the root and gets none.
//
//   ├─  a node with a later sibling
//   └─  the last sibling
//   │   carried down for each ancestor that still has siblings to come

export function treePrefixes(depths: number[]): string[] {
  const out: string[] = [];
  // more[k]: the ancestor at depth k+1 has later siblings.
  let more: boolean[] = [];
  depths.forEach((depth, i) => {
    if (depth === 0) {
      more = [];
      out.push("");
      return;
    }
    more = more.slice(0, depth - 1);
    const next = depths.slice(i + 1).find((d) => d <= depth);
    const last = next === undefined || next < depth;
    out.push(more.map((open) => (open ? "│  " : "   ")).join("") + (last ? "└─ " : "├─ "));
    more.push(!last);
  });
  return out;
}
