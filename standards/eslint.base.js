// Shared ESLint rules for every TypeScript app in the monorepo. CI runs
// `eslint . --max-warnings 0`, so every rule here is effectively an error.
//
// This file lives outside every package and cannot resolve node_modules
// itself, so each app's eslint.config.js imports its own plugin instances
// and passes them in. Apps may ADD rules after this config; they may not
// switch any of these off (see docs/CODING_STANDARDS.md).

/**
 * @param {object} deps
 * @param {any} deps.js - `@eslint/js`
 * @param {any} deps.tseslint - `typescript-eslint`
 * @param {any} deps.globals - `globals`
 * @param {string} deps.tsconfigRootDir - the app's `import.meta.dirname`
 * @param {any} [deps.reactHooks] - `eslint-plugin-react-hooks` (React apps)
 * @param {string[]} [deps.files] - typed source globs
 * @param {string[]} [deps.ignores] - extra ignore globs
 */
export default function strictConfig({
  js,
  tseslint,
  globals,
  tsconfigRootDir,
  reactHooks,
  files = ["src/**/*.{ts,tsx}"],
  ignores = [],
}) {
  return tseslint.config(
    { ignores: ["dist", "target", "src-tauri", "node_modules", ".astro", ...ignores] },
    js.configs.recommended,
    ...tseslint.configs.strictTypeChecked,
    ...tseslint.configs.stylisticTypeChecked,
    {
      files,
      languageOptions: {
        globals: globals.browser,
        parserOptions: { projectService: true, tsconfigRootDir },
      },
      plugins: reactHooks ? { "react-hooks": reactHooks } : {},
      rules: {
        ...(reactHooks ? reactHooks.configs.recommended.rules : {}),
        // The 400-line rule: files must stay readable in one sitting.
        "max-lines": ["error", { max: 400, skipBlankLines: false, skipComments: false }],
        "max-lines-per-function": ["error", { max: 120, skipBlankLines: true, skipComments: true }],
        "no-console": ["error", { allow: ["warn", "error"] }],
        "@typescript-eslint/no-unused-vars": ["error", { argsIgnorePattern: "^_" }],
        "@typescript-eslint/switch-exhaustiveness-check": "error",
        "@typescript-eslint/consistent-type-imports": "error",
        "@typescript-eslint/explicit-module-boundary-types": "error",
        eqeqeq: ["error", "always"],
      },
    },
    {
      // describe/it nesting makes function length meaningless in tests.
      files: ["**/*.test.ts", "**/*.test.tsx"],
      rules: { "max-lines-per-function": "off" },
    },
    {
      // Build plumbing isn't part of the typed program; lint it untyped.
      files: ["*.config.js", "*.config.ts", "*.config.mjs", "scripts/**/*.mjs"],
      ...tseslint.configs.disableTypeChecked,
      languageOptions: { globals: globals.node },
    },
  );
}
