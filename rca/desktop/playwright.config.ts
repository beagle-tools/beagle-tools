// Playwright e2e for the desktop frontend. The Tauri backend is mocked in the
// browser (src/e2e/mockIpc.ts), loaded only by `vite --mode e2e`, so these
// specs drive the real React app against fixture data — no Rust, no window.
import { defineConfig, devices } from "@playwright/test";

const PORT = 1430; // not 1420, so it can run beside `pnpm tauri dev`

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
    // `pnpm e2e` puts node_modules/.bin on PATH, so plain `vite` resolves.
    command: `vite --mode e2e --port ${String(PORT)} --strictPort`,
    url: `http://localhost:${String(PORT)}`,
    reuseExistingServer: !process.env.CI,
    timeout: 60_000,
  },
});
