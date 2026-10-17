// The agents monitor: daemon status and the agent list.
import { expect, test } from "./fixtures";

test("renders the agents view from the daemon status", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Agents", exact: true }).click();

  await expect(page.getByRole("heading", { name: "Agents" })).toBeVisible();
  await expect(page.locator(".agents-badge")).toContainText("beagle-agentd 0.29.0");
  const rows = page.locator(".agent-row");
  await expect(rows).toHaveCount(2);
  await expect(rows.nth(0)).toContainText("remediator");
  await expect(rows.nth(0)).toContainText("running");
  await expect(rows.nth(1)).toContainText("paused");

  await page.getByRole("button", { name: "← RCAs" }).click();
  await expect(page.locator(".sidebar-row")).toHaveCount(2);
});
