/// <reference types="vitest/config" />
import tailwindcss from "@tailwindcss/vite";
import react from "@vitejs/plugin-react";
import process from "node:process";
import { defineConfig } from "vite";

const host = process.env.TAURI_DEV_HOST;

// Vite options tailored for Tauri development:
// 1. clearScreen off so Rust errors stay visible
// 2. fixed port (Tauri expects it)
// 3. don't watch src-tauri
export default defineConfig(() => ({
  plugins: [react(), tailwindcss()],
  // Unit tests live beside the code; e2e/ is Playwright's and
  // scripts/*.test.mjs run under node:test (both via package.json).
  test: { include: ["src/**/*.test.{ts,tsx}"] },
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: {
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
}));
