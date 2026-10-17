// Telling bot-authored PRs from human ones. Dependabot, Renovate and
// friends open a steady stream of PRs that are real work but never
// urgent — keeping them out of the main list is the difference between
// a review queue you read and one you scroll past.

/** GitHub apps always carry the `[bot]` suffix; the rest is for named
 * service accounts ("dependabot", "renovate-bot", "ci-bot"). Matching
 * a bare "bot" anywhere would catch people (talbot, botros), so it has
 * to sit at a word edge or end the login. */
const BOT_PATTERNS = [/\[bot\]$/i, /(^|[-_.])bot([-_.]|$)/i, /[a-z]bot$/i];

export function isBotLogin(login: string): boolean {
  const name = login.trim().toLowerCase();
  if (!name) return false;
  return BOT_PATTERNS.some((pattern) => pattern.test(name));
}
