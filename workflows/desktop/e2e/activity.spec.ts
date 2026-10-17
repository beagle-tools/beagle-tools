// Activity: every chain on the machine, as a graph, as rows, and as the
// declared topology.
import { expect, test } from "@playwright/test";

test("the runs list shows every chain, roots first", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Activity" }).click();
  await page.getByRole("button", { name: "Flows" }).click();

  const rows = page.locator("ul.flows > li");
  await expect(rows).toHaveCount(4);
  await expect(rows.nth(0)).toContainText("demo/triage");
  await expect(rows.nth(1)).toContainText("demo/fix");
  await expect(rows.nth(2)).toContainText("demo/report");
  await expect(rows.nth(2)).toContainText("failed");
  // A failed step can be retried right from the list.
  await expect(rows.nth(2).getByRole("button", { name: "Retry" })).toBeVisible();
  await expect(page.getByText("2 chains")).toBeVisible();
});

test("the graph draws one node per run", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Activity" }).click();

  await expect(page.locator(".gnode")).toHaveCount(4);
  await expect(page.locator(".gnode.failed")).toHaveCount(1);
});

test("topology lists declared agents and opens a manifest", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Activity" }).click();
  await page.getByRole("button", { name: "Topology" }).click();

  await expect(page.locator(".topology .node")).toHaveCount(4);
  await page.locator(".topology .node", { hasText: "triage" }).click();
  await expect(page.locator(".config")).toContainText("runner: claude");
});
