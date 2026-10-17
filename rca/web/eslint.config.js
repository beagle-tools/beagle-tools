// Lint bar for the postmortem site: the shared monorepo rules
// (standards/eslint.base.js) on the typed TypeScript sources, plus
// eslint-plugin-astro's recommended rules for .astro components.
// `pnpm lint` runs with --max-warnings 0, so every rule is an error.
import js from "@eslint/js";
import astro from "eslint-plugin-astro";
import globals from "globals";
import tseslint from "typescript-eslint";
import strictConfig from "../../standards/eslint.base.js";

export default [
  ...strictConfig({
    js,
    tseslint,
    globals,
    tsconfigRootDir: import.meta.dirname,
    files: ["src/**/*.ts"],
  }),
  ...astro.configs.recommended,
  {
    // Component frontmatter is TypeScript too: give the shared typed
    // rules the type information they need there.
    files: ["**/*.astro"],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser,
        project: "./tsconfig.json",
        extraFileExtensions: [".astro"],
        tsconfigRootDir: import.meta.dirname,
      },
    },
  },
];
