// Tandem's ESLint config: the shared monorepo rules (standards/eslint.base.js)
// with this app's own plugin instances. CI runs with --max-warnings 0.
import js from "@eslint/js";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

import strictConfig from "../standards/eslint.base.js";

export default [
  ...strictConfig({ js, tseslint, globals, reactHooks, tsconfigRootDir: import.meta.dirname }),
];
