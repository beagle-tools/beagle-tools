// The RCA browser: listing, opening a workspace, sections, diagrams, PRs.
import { expect, test } from "./fixtures";

test.beforeEach(async ({ page }) => {
  await page.goto("/");
});

test("lists every workspace in the sidebar", async ({ page }) => {
  const rows = page.locator(".sidebar-row");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText("Cache stampede after a deploy");
  await expect(rows.nth(1)).toContainText("Checkout timeouts during a config rollout");
});

test("opens a workspace and renders its sections", async ({ page }) => {
  await page.getByRole("button", { name: /Checkout timeouts during a config rollout/ }).click();

  await expect(page.getByRole("heading", { level: 1 })).toHaveText(
    "Checkout timeouts during a config rollout",
  );
  await expect(page.locator(".incident-meta")).toContainText("finished · high");
  await expect(page.getByText(/roughly 8% of checkout attempts timed out/)).toBeVisible();

  await page.getByRole("tab", { name: /Timeline/ }).click();
  await expect(page.getByRole("tab", { name: /Timeline/ })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expect(page.getByText(/the configuration change is rolled back/)).toBeVisible();

  await page.getByRole("tab", { name: /Root Cause/ }).click();
  await expect(page.getByText(/connection pool had been reduced/)).toBeVisible();
});

test("renders diagrams", async ({ page }) => {
  await page.getByRole("button", { name: /Checkout timeouts/ }).click();
  await page.getByRole("tab", { name: /Diagrams/ }).click();
  await expect(page.getByText(/pool: too small/)).toBeVisible();
});

test("shows attached PRs with their live state", async ({ page }) => {
  await page.getByRole("button", { name: /Checkout timeouts/ }).click();
  const chip = page.locator(".pr-chip");
  await expect(chip).toHaveCount(1);
  await expect(chip).toContainText("#4212");
  await expect(chip).toContainText("merged");
});
