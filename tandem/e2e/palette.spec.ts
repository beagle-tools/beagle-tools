// ⌘K / Ctrl+K command palette: fuzzy-jump to a loaded PR.
import { expect, test } from "@playwright/test";

test("command palette opens via shortcut and jumps to a PR", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Cache rendered markdown per comment")).toBeVisible();

  await page.keyboard.press("ControlOrMeta+k");
  const input = page.getByRole("textbox").and(page.locator(":focus"));
  await expect(input).toBeVisible();

  await input.fill("markdown");
  await input.press("Enter");

  await expect(
    page.getByRole("heading", { name: /Cache rendered markdown per comment/ }),
  ).toBeVisible();
});
