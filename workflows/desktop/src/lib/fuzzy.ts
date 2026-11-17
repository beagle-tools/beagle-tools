// fzf-style fuzzy matching for the jobs page: the query's characters must
// appear in order, not necessarily together. `chop` finds
// cluster-health-opa; `trp` finds ticket-report.
//
// Scored so the match a person meant ranks first: consecutive characters
// and characters at the start of a word (after `-`, `_`, `/`, a space, or
// the start) score high; every gap costs a little. Case-insensitive.

export interface FuzzyMatch {
  score: number;
  /** Indices into the target that matched, for highlighting. */
  positions: number[];
}

const BOUNDARY = new Set(["-", "_", "/", " ", "."]);

/** Score `target` against `query`, or `null` when it does not match. */
export function fuzzyMatch(query: string, target: string): FuzzyMatch | null {
  const q = query.trim().toLowerCase();
  if (q === "") return { score: 0, positions: [] };
  const t = target.toLowerCase();

  // Greedy left-to-right, then a pass from the right to tighten the span:
  // `ch` in "cluster-health" should take the `h` of "health", not of
  // "cluster" — the tighter span reads as the intended match.
  const positions: number[] = [];
  let ti = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, ti);
    if (found === -1) return null;
    positions.push(found);
    ti = found + 1;
  }
  let end = positions[positions.length - 1] ?? 0;
  for (let qi = q.length - 1; qi >= 0; qi--) {
    const found = t.lastIndexOf(q.charAt(qi), end);
    positions[qi] = found;
    end = found - 1;
  }

  let score = 0;
  positions.forEach((pos, i) => {
    score += 1;
    if (pos === 0 || BOUNDARY.has(t.charAt(pos - 1))) score += 8;
    const prev = positions[i - 1];
    if (prev !== undefined) {
      const gap = pos - prev - 1;
      score += gap === 0 ? 5 : -Math.min(gap, 6);
    }
  });
  // An exact substring beats the same letters scattered.
  if (t.includes(q)) score += 10;
  // Shorter targets win ties: `scan` should prefer cluster-health-scan over
  // a long name that happens to contain the letters.
  score -= target.length / 100;
  return { score, positions };
}

/** Split `text` into runs of matched / unmatched characters for rendering. */
export function highlightRuns(text: string, positions: number[]): { text: string; hit: boolean }[] {
  const hits = new Set(positions);
  const out: { text: string; hit: boolean }[] = [];
  for (let i = 0; i < text.length; i++) {
    const hit = hits.has(i);
    const last = out[out.length - 1];
    if (last?.hit === hit) last.text += text.charAt(i);
    else out.push({ text: text.charAt(i), hit });
  }
  return out;
}
