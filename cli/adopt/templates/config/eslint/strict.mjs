import { defineConfig } from "eslint/config";
import tseslint from "typescript-eslint";
import standard from "./standard.mjs";

const typed = process.env.DX_ESLINT_TYPED === "1";

export default defineConfig(standard, {
  files: ["**/*.ts", "**/*.tsx", "**/*.mts", "**/*.cts"],
  extends: [
    tseslint.configs.strictTypeChecked,
    tseslint.configs.stylisticTypeChecked,
    ...(typed ? [] : [tseslint.configs.disableTypeChecked]),
  ],
});
