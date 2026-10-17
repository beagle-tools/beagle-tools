// Run job: required parameters gate the Run button, and the run lands.
import { expect, test } from "@playwright/test";

test("starting a job with parameters queues a run", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Run job" }).click();

  const modal = page.locator(".modal");
  await modal.locator("button.job", { hasText: "hello" }).click();
  const run = modal.getByRole("button", { name: "Run", exact: true });
  await expect(run).toBeDisabled();
  await expect(modal).toContainText("needs name");

  await modal.getByPlaceholder("who to greet").fill("Ada");
  await modal.getByPlaceholder("manifest default").fill("haiku");
  await expect(run).toBeEnabled();
  await run.click();
  await expect(modal).toHaveCount(0);

  const sent = await page.evaluate(() => window.__agentdeskE2E?.triggers);
  expect(sent).toEqual([
    { project: "demo", agent: "hello", params: { name: "Ada" }, model: "haiku" },
  ]);
  // The job now shows as running, and the new run is in Activity.
  await expect(page.locator(".jobblock", { hasText: "hello" })).toContainText("running · hello");
  await page.getByRole("button", { name: "Activity" }).click();
  await page.getByRole("button", { name: "Flows" }).click();
  await expect(page.locator("ul.flows > li").first()).toContainText("demo/hello");
  await expect(page.locator("ul.flows > li").first()).toContainText("pending");
});

test("cancel closes the launcher without queueing anything", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Run job" }).click();
  await page.locator(".modal").getByRole("button", { name: "Cancel" }).click();

  await expect(page.locator(".modal")).toHaveCount(0);
  expect(await page.evaluate(() => window.__agentdeskE2E?.triggers.length)).toBe(0);
});
