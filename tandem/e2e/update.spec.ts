// The header update button: hidden while the updater reports nothing new,
// shown once it does (the mock's `?update=` knob).
import { expect, test } from "@playwright/test";

test("update button is hidden when no update is available", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Add retry with jitter to the webhook dispatcher")).toBeVisible();

  await expect(page.getByRole("button", { name: /^update v/ })).toHaveCount(0);
});

test("update button appears when an update is available", async ({ page }) => {
  await page.goto("/?update=0.5.0");

  await expect(page.getByRole("button", { name: "update v0.5.0" })).toBeVisible();
});
