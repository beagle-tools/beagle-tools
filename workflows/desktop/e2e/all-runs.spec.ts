// All runs: every job's chains on one page, and the Running filter.
import { expect, test } from "@playwright/test";

test("all runs lists every job's chains, newest first", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "All runs" }).click();

  await expect(page.locator(".chainblock")).toHaveCount(2);
  await expect(page.locator(".chainblock").first()).toContainText("demo/triage");
  await expect(page.getByText("2 runs")).toBeVisible();
});

test("the jobs page fuzzy-searches job names", async ({ page }) => {
  await page.goto("/");
  await page.getByPlaceholder(/Fuzzy-search jobs/).fill("trg");

  await expect(page.locator(".jobblock")).toHaveCount(1);
  await expect(page.locator(".jobblock")).toContainText("triage");
  await expect(page.getByText("1 of 2 · enter opens the first")).toBeVisible();
});
