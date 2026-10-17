// The in-app updater: the pill only appears when a newer release exists,
// and the version list offers update / rollback per release.
import { expect, test } from "./fixtures";

test("hides the update pill when the app is up to date", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator(".sidebar-row")).toHaveCount(2);
  await expect(page.getByRole("button", { name: "v0.29.0" })).toBeVisible();
  await expect(page.getByTestId("update-pill")).toHaveCount(0);
});

test("shows the update pill when a newer release exists", async ({ page }) => {
  await page.addInitScript(() => {
    window.__BEAGLE_E2E__ = { update: "0.30.0" };
  });
  await page.goto("/");
  await expect(page.getByTestId("update-pill")).toHaveText("update v0.30.0");
});

test("lists releases with update and rollback actions", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "v0.29.0" }).click();

  const dialog = page.getByRole("dialog", { name: "Versions" });
  await expect(dialog).toBeVisible();
  const rows = dialog.locator(".version-row");
  await expect(rows).toHaveCount(3);
  await expect(rows.nth(0)).toContainText("beagle-rca-v0.30.0");
  await expect(rows.nth(0).getByRole("button", { name: "update" })).toBeVisible();
  await expect(rows.nth(1)).toContainText("current");
  await expect(rows.nth(2).getByRole("button", { name: "roll back" })).toBeVisible();

  await page.keyboard.press("Escape");
  await expect(dialog).toHaveCount(0);
});
