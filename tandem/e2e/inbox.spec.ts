// Boot: the app starts on Review status with the account-wide queues
// filled from the mocked GitHub data.
import { expect, test } from "@playwright/test";

test("boots to the review-status inbox with requested PRs", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByText("Review status")).toBeVisible();
  await expect(page.getByText("Add retry with jitter to the webhook dispatcher")).toBeVisible();
  await expect(page.getByText("Cache rendered markdown per comment")).toBeVisible();
});

test("switches inbox tabs to the viewer's own PRs", async ({ page }) => {
  await page.goto("/");

  await page.getByRole("button", { name: /^mine/ }).click();
  await expect(page.getByText("Bump the SQLite schema to v9")).toBeVisible();
  await expect(page.getByText("Cache rendered markdown per comment")).toBeHidden();
});
