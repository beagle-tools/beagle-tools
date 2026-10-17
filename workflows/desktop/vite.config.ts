/// <reference types="vitest/config" />
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// 1421, not Tauri's default 1420: the Tandem dev server already owns that
// port, and both apps are often open at once.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1421,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  // Unit tests live beside the code; e2e/ is Playwright's and
  // ../scripts/*.test.mjs run under node:test (both via package.json).
  test: { include: ["src/**/*.test.{ts,tsx}"] },
});
