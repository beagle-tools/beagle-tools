// Builds on the monorepo's shared strict config. This file may ADD rules;
// it may not switch any shared rule off (scripts/check-standards.mjs).
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

import strictConfig from "../../standards/eslint.base.js";

export default [
  ...strictConfig({
    js,
    tseslint,
    globals,
    reactHooks,
    tsconfigRootDir: import.meta.dirname,
    // Playwright specs are typed TS under the same rules as the app.
    files: ["src/**/*.{ts,tsx}", "e2e/**/*.ts"],
    ignores: ["test-results", "playwright-report", "blob-report"],
  }),
];
