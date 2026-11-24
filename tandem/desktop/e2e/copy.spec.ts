// Copy-to-clipboard affordances: a PR's link from its list row (without
// opening the PR), and a local thread as markdown.
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";

test.use({ permissions: ["clipboard-read", "clipboard-write"] });

const clipboard = (page: Page): Promise<string> =>
  page.evaluate(() => navigator.clipboard.readText());

test("copies a PR's URL from its row without opening it", async ({ page }) => {
  await page.goto("/");
  const title = "Add retry with jitter to the webhook dispatcher";
  await expect(page.getByText(title)).toBeVisible();

  // The row's own button opens the PR; the copy icon sits beside it.
  const row = page.getByRole("button").filter({ hasText: title }).first().locator("xpath=..");
  await row.getByTitle("copy the PR URL").click();

  await expect(page.getByText("PR URL copied")).toBeVisible();
  expect(await clipboard(page)).toBe("https://github.com/acme/widgets/pull/128");
  // Still on the list: copying never opens the PR.
  await expect(page.getByText("Review status")).toBeVisible();
});

test("copies a local thread as a markdown conversation", async ({ page }) => {
  await page.goto("/");
  await page.getByText("Add retry with jitter to the webhook dispatcher").click();

  await page.getByTitle("copy this whole thread as markdown").first().click();

  await expect(page.getByText("conversation copied")).toBeVisible();
  const text = await clipboard(page);
  expect(text).toMatch(/^### \S+:\d+/);
  expect(text).toContain("**");
});
