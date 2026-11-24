// The Jobs page (the landing page) and one job's shape.
import { expect, test } from "@playwright/test";

test("lists each project's jobs with their flow and last result", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("heading", { name: "demo" })).toBeVisible();
  const triage = page.locator(".jobblock", { hasText: "triage" });
  await expect(triage).toContainText("cron 0 9 * * 1-5");
  await expect(triage).toContainText("succeeded");
  // The flow beneath the job: its two downstream steps.
  await expect(triage.locator(".flowtree")).toContainText("fix");
  await expect(triage.locator(".flowtree")).toContainText("report");

  const hello = page.locator(".jobblock", { hasText: "hello" });
  await expect(hello).toContainText("manual");
  await expect(hello).toContainText("never run");
});

test("a job's Shape tab draws every branch it has taken", async ({ page }) => {
  await page.goto("/");
  await page.locator(".jobblock", { hasText: "triage" }).click();

  await expect(page.getByRole("heading", { name: "demo / triage" })).toBeVisible();
  await page.getByRole("button", { name: "Shape" }).click();
  const tree = page.locator(".flowtree.big");
  await expect(tree).toContainText("triage");
  await expect(tree).toContainText("fix");
  await expect(tree).toContainText("report");
});

test("f shows only running jobs, a shows every job again, r does nothing", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".jobblock")).toHaveCount(2);
  const all = page.getByRole("button", { name: "All", exact: true });
  const running = page.getByRole("button", { name: /^Running/ });

  await page.keyboard.press("r");
  await expect(all).toHaveAttribute("aria-pressed", "true");

  await page.keyboard.press("f");
  await expect(running).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".jobblock")).toHaveCount(0);
  await expect(page.getByText("Nothing running right now")).toBeVisible();

  await page.keyboard.press("a");
  await expect(all).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator(".jobblock")).toHaveCount(2);
});
