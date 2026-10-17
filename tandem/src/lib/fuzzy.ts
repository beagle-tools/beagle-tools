// Lightweight fuzzy matching for the PR filter (fzf-style): query chars
// must appear in order, contiguous substrings rank highest, tighter
// subsequences beat scattered ones. Returns null when there's no match.

export function fuzzyScore(query: string, text: string): number | null {
  const q = query.toLowerCase().replace(/\s+/g, "");
  const t = text.toLowerCase();
  if (!q) return 0;

  const idx = t.indexOf(query.trim().toLowerCase());
  if (idx >= 0) return 1000 - idx;

  let ti = 0;
  let score = 0;
  for (const ch of q) {
    const found = t.indexOf(ch, ti);
    if (found < 0) return null;
    // Adjacent hits score higher than scattered ones.
    score += found === ti ? 5 : 1;
    ti = found + 1;
  }
  return score;
}
