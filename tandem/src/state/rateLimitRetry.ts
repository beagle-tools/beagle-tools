// GitHub's rate limit is a "come back shortly", not a failure. These
// list loads fire together on the review-status page, which is exactly
// when a limit gets tripped — so when one does, say so and try again
// rather than leaving a half-loaded page behind a red toast.

import { pushInfo } from "./toasts";

const RATE_LIMITED = /rate limit|secondary rate|abuse detection/i;

/** Long enough for a secondary limit to clear, short enough that the
 * page fills in while you're still looking at it. */
export const RETRY_AFTER_MS = 30_000;

export async function withRateLimitRetry<T>(run: () => Promise<T>, attempts = 2): Promise<T> {
  for (let tries = 0; ; tries++) {
    try {
      return await run();
    } catch (e) {
      const message = e instanceof Error ? e.message : String(e);
      if (tries >= attempts || !RATE_LIMITED.test(message)) throw e;
      pushInfo("GitHub rate limit — trying again in a bit");
      await new Promise((resolve) => setTimeout(resolve, RETRY_AFTER_MS));
    }
  }
}
