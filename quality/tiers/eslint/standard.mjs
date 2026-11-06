import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import globals from "globals";
import tseslint from "typescript-eslint";
import frameworks from "./frameworks.mjs";

const typed = process.env.DX_ESLINT_TYPED === "1";
const typescriptFiles = ["**/*.ts", "**/*.tsx", "**/*.mts", "**/*.cts"];
const javascriptFiles = ["**/*.js", "**/*.jsx", "**/*.mjs", "**/*.cjs"];

export default defineConfig(
	js.configs.recommended,
	{ languageOptions: { globals: { ...globals.browser, ...globals.node } } },
	{ files: javascriptFiles, rules: { "no-unused-vars": "error" } },
	{
		files: typescriptFiles,
		extends: [
			tseslint.configs.recommendedTypeChecked,
			...(typed ? [] : [tseslint.configs.disableTypeChecked]),
		],
		languageOptions: typed
			? {
					parserOptions: {
						projectService: true,
						tsconfigRootDir: process.cwd(),
					},
				}
			: {},
		rules: {
			"no-unused-vars": "off",
			"@typescript-eslint/no-unused-vars": "error",
		},
	},
	frameworks,
);
