import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import standard from "./standard.mjs";

export default defineConfig(standard, {
  files: ["**/*.ts", "**/*.tsx", "**/*.mts", "**/*.cts"],
  extends: [tseslint.configs.strict, tseslint.configs.stylistic],
});
