// App updates: the top-bar pill and the Versions dialog.
import { expect, test } from "@playwright/test";

test("the update pill is hidden when no update is available", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".jobblock").first()).toBeVisible();
  // The version button is there, so the update check has had its chance.
  await expect(page.getByRole("button", { name: "v0.2.0" })).toBeVisible();

  await expect(page.getByTestId("update-pill")).toHaveCount(0);
});

test("the update pill appears when the mirror offers a newer version", async ({ page }) => {
  await page.goto("/?update=0.3.0");

  await expect(page.getByTestId("update-pill")).toHaveText("Update to v0.3.0");
});

test("the Versions dialog offers update and rollback per release", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "v0.2.0" }).click();

  const dialog = page.getByRole("dialog", { name: "Versions" });
  const rows = dialog.locator(".release");
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toContainText("beagle-workflows-v0.3.0");
  await expect(rows.nth(0).getByRole("button", { name: "Update" })).toBeVisible();
  await expect(rows.nth(1)).toContainText("current");
  await expect(rows.nth(2).getByRole("button", { name: "Roll back" })).toBeVisible();

  await dialog.getByRole("button", { name: "Close" }).click();
  await expect(dialog).toHaveCount(0);
});
