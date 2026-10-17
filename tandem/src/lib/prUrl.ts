// Parses a pasted GitHub PR URL into a repo slug + number, for
// jumping straight to a PR Tandem hasn't necessarily loaded yet.

const PR_URL_RE = /^https?:\/\/github\.com\/([\w.-]+)\/([\w.-]+)\/pull\/(\d+)/i;

export function parsePrUrl(input: string): { slug: string; number: number } | null {
  const match = PR_URL_RE.exec(input.trim());
  const owner = match?.[1];
  const name = match?.[2];
  const number = match?.[3];
  if (!owner || !name || !number) return null;
  return { slug: `${owner}/${name}`, number: Number(number) };
}
