// Opening a PR: header, diff and both kinds of comments (local agent
// findings and GitHub review comments) render from the cached bundle.
import { expect, test } from "@playwright/test";

test("opening a PR renders its diff and comments", async ({ page }) => {
  await page.goto("/");
  await page.getByText("Add retry with jitter to the webhook dispatcher").click();

  await expect(
    page.getByRole("heading", { name: /Add retry with jitter to the webhook dispatcher/ }),
  ).toBeVisible();

  // Diff: both files and the added lines.
  const diff = page.locator("#diff-scroll");
  await expect(diff.getByText("src/webhooks/dispatch.rs").first()).toBeVisible();
  await expect(diff.getByText("src/webhooks/backoff.rs").first()).toBeVisible();
  await expect(diff.getByText("let mut attempt = 0;")).toBeVisible();

  // Comments: the agent's local finding and the GitHub thread.
  await expect(page.getByText("comments (2)")).toBeVisible();
  await expect(
    page.getByText("The retry loop never sleeps between attempts").first(),
  ).toBeVisible();
  await expect(
    page.getByText("Should MAX_RETRIES be configurable per hook?").first(),
  ).toBeVisible();
});
