// Settings › Agents: renaming an agent moves it instead of leaving a
// copy behind under the old name.
import { expect, test } from "@playwright/test";

test("renaming an agent replaces it rather than cloning it", async ({ page }) => {
  await page.goto("/");
  await expect(page.getByText("Review status")).toBeVisible();
  await page.keyboard.press("ControlOrMeta+,");

  const agents = page
    .locator("section")
    .filter({ has: page.getByRole("heading", { name: "Agents" }) });
  await agents.getByRole("button", { name: "edit" }).click();

  const name = agents.getByLabel("name", { exact: true });
  await expect(name).toHaveValue("claude-reviewer");
  await name.fill("careful-reviewer");
  await agents.getByRole("button", { name: "save agent" }).click();

  await expect(agents.getByText("careful-reviewer")).toBeVisible();
  await expect(agents.getByText("claude-reviewer")).toBeHidden();
});
