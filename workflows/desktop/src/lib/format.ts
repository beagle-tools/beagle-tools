// Display helpers. Everything here takes the raw serde value and returns
// something for a cell — no component formats a timestamp itself.

export function age(iso: string | null): string {
  if (!iso) return "—";
  const then = new Date(iso).getTime();
  if (Number.isNaN(then)) return "—";
  return short(Math.max(0, Date.now() - then) / 1000);
}

export function duration(startedAt: string | null, finishedAt: string | null): string {
  if (!startedAt) return "—";
  const start = new Date(startedAt).getTime();
  const end = finishedAt ? new Date(finishedAt).getTime() : Date.now();
  if (Number.isNaN(start) || Number.isNaN(end)) return "—";
  return short(Math.max(0, end - start) / 1000);
}

// Two significant units at most: "1h 4m", not "1h 4m 12s". Precision
// nobody reads costs column width everybody does.
function short(totalSeconds: number): string {
  const s = Math.floor(totalSeconds);
  if (s < 60) return unit(s, "s");
  const m = Math.floor(s / 60);
  if (m < 60) return s % 60 ? `${unit(m, "m")} ${unit(s % 60, "s")}` : unit(m, "m");
  const h = Math.floor(m / 60);
  if (h < 24) return m % 60 ? `${unit(h, "h")} ${unit(m % 60, "m")}` : unit(h, "h");
  const d = Math.floor(h / 24);
  return h % 24 ? `${unit(d, "d")} ${unit(h % 24, "h")}` : unit(d, "d");
}

function unit(n: number, suffix: string): string {
  return String(n) + suffix;
}

// Sub-cent runs are common, and "$0.00" reads as free when it is not.
export function cost(usd: number | null): string {
  if (usd === null) return "—";
  if (usd > 0 && usd < 0.01) return "<$0.01";
  return `$${usd.toFixed(2)}`;
}

export function params(json: string | null): Record<string, string> {
  if (!json) return {};
  try {
    const parsed: unknown = JSON.parse(json);
    if (typeof parsed !== "object" || parsed === null) return {};
    const out: Record<string, string> = {};
    for (const [k, v] of Object.entries(parsed)) {
      if (typeof v === "string") out[k] = v;
    }
    return out;
  } catch {
    return {};
  }
}
