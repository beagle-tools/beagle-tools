// Smoke tests against the built site, using the published example in
// rca/rcas/2026-05-12-example-checkout-timeouts.
import { expect, test } from "./fixtures";

const SLUG = "2026-05-12-example-checkout-timeouts";
const TITLE = "Checkout timeouts during a config rollout";

test("the index lists published incidents", async ({ page }) => {
  await page.goto("/");
  await expect(page).toHaveTitle(/Incident History/);
  await expect(page.getByRole("heading", { level: 1 })).toHaveText("Incident history");
  await expect(page.getByRole("heading", { level: 2, name: TITLE })).toBeVisible();
});

test("an incident page renders its public sections and diagram", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("heading", { level: 2, name: TITLE }).click();
  await expect(page).toHaveURL(new RegExp(`/incidents/${SLUG}/?$`));

  await expect(page.getByRole("heading", { level: 1 })).toHaveText(TITLE);
  for (const section of ["Timeline", "What happened", "Impact", "Resolution", "Diagrams"]) {
    await expect(page.getByRole("heading", { level: 2, name: section })).toBeVisible();
  }
  await expect(page.getByText(/the configuration change is rolled back/)).toBeVisible();
  await expect(page.getByText(/pool: too small/)).toBeVisible();
});

test("internal sections are never published", async ({ page }) => {
  await page.goto(`/incidents/${SLUG}/`);
  for (const internal of ["Notes", "Log", "Final Review"]) {
    await expect(page.getByRole("heading", { level: 2, name: internal })).toHaveCount(0);
  }
});
