// Realistic fixture data for the Playwright e2e run (`vite --mode e2e`).
// Modelled on rcas/2026-05-12-example-checkout-timeouts plus an in-flight
// investigation with an attached PR. Never part of the production bundle:
// main.tsx only imports this tree when import.meta.env.MODE === "e2e".

import type { CorpusLine } from "../lib/finder";
import type { AgentsStatus, AppRelease, Listing } from "../types";

export const CHECKOUT_ID = "2026-05-12-example-checkout-timeouts";
export const CACHE_ID = "2026-10-14-cache-stampede-on-deploy";
export const PR_URL = "https://github.com/acme/checkout/pull/4212";

export const LISTING: Listing = {
  root: "/Users/e2e/oncall",
  workspaces: [
    {
      id: CACHE_ID,
      title: "Cache stampede after a deploy",
      severity: "critical",
      status: "investigating",
      created: "2026-10-14T09:12:00Z",
      systems: ["edge-cache", "catalog-api"],
      tags: ["deploy"],
      prs: [],
      archived: false,
    },
    {
      id: CHECKOUT_ID,
      title: "Checkout timeouts during a config rollout",
      severity: "high",
      status: "finished",
      created: "2026-07-17T20:39:43Z",
      systems: ["checkout-api", "config-service"],
      tags: [],
      prs: [PR_URL],
      archived: false,
    },
  ],
  broken: [],
  warnings: [],
};

export const SECTIONS: Record<string, Record<string, string>> = {
  [CHECKOUT_ID]: {
    "summary.md": `# Summary

For 34 minutes on 2026-05-12, roughly 8% of checkout attempts timed out
after a configuration rollout narrowed a connection pool below what peak
traffic needed. We rolled the change back and checkout recovered fully.
`,
    "timeline.md": `# Timeline

- **18:02 UTC** — a routine configuration change rolls out to checkout-api.
- **18:07 UTC** — checkout timeout rate begins climbing.
- **18:24 UTC** — the configuration change is rolled back.
- **18:36 UTC** — timeout rate returns to normal; incident resolved.
`,
    "root-cause.md": `# What happened

1. Checkout requests began timing out under normal peak traffic.
2. The connection pool had been reduced below what peak checkout needs.
`,
  },
  [CACHE_ID]: {
    "summary.md": "# Summary\n\nCatalog p99 spiked to 9s right after the 09:05 deploy.\n",
  },
};

export const DIAGRAMS: Record<string, Record<string, string>> = {
  [CHECKOUT_ID]: {
    "01-request-path.txt": `client ─▶ checkout-api ─▶ [pool: too small] BUG
                 │              waiting…
                 ▼
             timeout (8% of attempts)`,
  },
};

export const CORPUS: CorpusLine[] = [
  {
    id: CHECKOUT_ID,
    title: "Checkout timeouts during a config rollout",
    file: "summary.md",
    line: 3,
    text: "after a configuration rollout narrowed a connection pool below what peak",
  },
];

export const AGENTS: AgentsStatus = {
  version: "0.29.0",
  agents: [
    {
      id: "remediator",
      enabled: true,
      running: true,
      last_tick: "2026-10-16T08:00:00Z",
      active_sessions: 1,
      last_results: ["opened acme/checkout#4212 for 2026-05-12-example-checkout-timeouts"],
    },
    {
      id: "triage",
      enabled: true,
      running: false,
      last_tick: null,
      active_sessions: 0,
      last_results: [],
    },
  ],
};

export const RELEASES: AppRelease[] = [
  {
    version: "0.30.0",
    tag: "beagle-rca-v0.30.0",
    published_at: "2026-10-15T12:00:00Z",
    installable: true,
  },
  {
    version: "0.29.0",
    tag: "beagle-rca-v0.29.0",
    published_at: "2026-10-01T12:00:00Z",
    installable: true,
  },
  {
    version: "0.28.0",
    tag: "beagle-rca-v0.28.0",
    published_at: "2026-09-20T12:00:00Z",
    installable: true,
  },
];
