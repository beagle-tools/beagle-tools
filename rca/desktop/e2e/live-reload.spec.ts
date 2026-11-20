// Live reload: when a file under rcas/ changes, the backend's watcher fires
// `workspaces-changed` and the open section re-renders without a click.
import { expect, test } from "./fixtures";

const CHECKOUT_ID = "2026-05-12-example-checkout-timeouts";

test("an edited section updates the open view", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: /Checkout timeouts during a config rollout/ }).click();
  await expect(page.getByText(/roughly 8% of checkout attempts timed out/)).toBeVisible();

  await page.evaluate(async (id) => {
    await window.__beagleE2eWrite?.(id, "summary.md", "# Summary\n\nAn agent just rewrote this.\n");
  }, CHECKOUT_ID);

  await expect(page.getByText("An agent just rewrote this.")).toBeVisible();
  await expect(page.getByText(/roughly 8% of checkout attempts timed out/)).toHaveCount(0);
});
