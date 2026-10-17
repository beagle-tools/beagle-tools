// Lint bar for the desktop frontend: the shared monorepo rules
// (standards/eslint.base.js) plus React hooks. `pnpm lint` runs with
// --max-warnings 0, so every rule is effectively an error.
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
  }),
];
