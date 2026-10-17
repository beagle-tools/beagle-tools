// Playwright e2e for Tandem: the real React frontend in Chromium, served by
// vite in `e2e` mode, which swaps every Tauri command for fixture data
// (src/e2e/mockIpc.ts). Port 1430 keeps clear of `make dev` (1420).
import { defineConfig, devices } from "@playwright/test";

const PORT = 1430;
const ci = Boolean(process.env.CI);

export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  forbidOnly: ci,
  retries: ci ? 1 : 0,
  reporter: ci ? [["list"], ["html", { open: "never" }]] : "list",
  use: {
    baseURL: `http://localhost:${String(PORT)}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `vite --mode e2e --port ${String(PORT)} --strictPort`,
    url: `http://localhost:${String(PORT)}`,
    reuseExistingServer: !ci,
    timeout: 60_000,
  },
});
