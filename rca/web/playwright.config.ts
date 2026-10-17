// Playwright e2e for the postmortem site: builds the static site from the
// repo's rcas/ (only `published = true` workspaces) and serves dist/ with
// `astro preview`, so the specs exercise exactly what ships.
import { defineConfig, devices } from "@playwright/test";

const PORT = 4330;

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: process.env.CI ? [["list"], ["github"]] : "list",
  use: {
    baseURL: `http://localhost:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    // `pnpm e2e` puts node_modules/.bin on PATH, so plain `astro` resolves.
    command: `astro build && astro preview --port ${String(PORT)}`,
    url: `http://localhost:${String(PORT)}`,
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
});
