// Settings, including Settings › Version (the in-app rollback list, which
// only lists this app's `beagle-tandem-v*` releases).
import { expect, test } from "@playwright/test";

test("settings view renders with the version list", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Review status")).toBeVisible();

  await page.keyboard.press("ControlOrMeta+,");

  await expect(page.getByRole("heading", { name: "Version" })).toBeVisible();
  await expect(page.getByText("v0.4.2", { exact: true })).toBeVisible();
  const current = page.getByText("beagle-tandem-v0.4.2").locator("xpath=..");
  await expect(current.getByText("current")).toBeVisible();
  const newer = page.getByText("beagle-tandem-v0.4.3").locator("xpath=../..");
  await expect(newer.getByRole("button", { name: "update" })).toBeVisible();
  const older = page.getByText("beagle-tandem-v0.4.1").locator("xpath=../..");
  await expect(older.getByRole("button", { name: "roll back" })).toBeVisible();
});
