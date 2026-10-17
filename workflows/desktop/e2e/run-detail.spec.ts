// One job's runs: pick a step, read its log.
import { expect, test } from "@playwright/test";

test("opens on the newest run's first step and shows its conversation", async ({ page }) => {
  await page.goto("/");
  await page.locator(".jobblock", { hasText: "triage" }).click();

  await expect(page.getByRole("button", { name: "Runs (2)" })).toBeVisible();
  await expect(page.locator(".chainblock")).toHaveCount(2);
  const pane = page.locator(".steppane");
  await expect(pane.locator(".stephead")).toContainText("triage");
  await expect(pane).toContainText("conversation");
  await expect(pane).toContainText("Looking at the open issues labelled bug.");
  await expect(pane).toContainText("4 lines");
});

test("clicking a failed step shows its script log and a retry", async ({ page }) => {
  await page.goto("/");
  await page.locator(".jobblock", { hasText: "triage" }).click();
  await page.locator(".steprows li", { hasText: "report" }).click();

  const pane = page.locator(".steppane");
  await expect(pane.locator(".stephead")).toContainText("report");
  await expect(pane.locator(".stephead")).toContainText("exit 1");
  await expect(pane).toContainText("script log");
  await expect(pane).toContainText("slack webhook returned 403");
  await expect(pane.getByRole("button", { name: "Retry" })).toBeVisible();
});

test("the log search filters lines", async ({ page }) => {
  await page.goto("/");
  await page.locator(".jobblock", { hasText: "triage" }).click();
  await page.getByPlaceholder("Search…").fill("gh issue");

  await expect(page.locator(".steppane")).toContainText("1 of 4");
  await expect(page.locator(".transcript .le")).toHaveCount(1);
});
