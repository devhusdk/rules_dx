import svelte from "eslint-plugin-svelte";
import tseslint from "typescript-eslint";

export default [
	...svelte.configs.recommended,
	{
		files: ["**/*.svelte"],
		languageOptions: { parserOptions: { parser: "@typescript-eslint/parser" } },
	},
	{
		files: ["**/*.svelte"],
		plugins: { "@typescript-eslint": tseslint.plugin },
		rules: {
			"no-unused-vars": "off",
			"@typescript-eslint/no-unused-vars": "error",
		},
	},
];

export const strict = [
	{
		files: ["**/*.svelte"],
		rules: {
			"svelte/button-has-type": "error",
			"svelte/derived-has-same-inputs-outputs": "error",
			"svelte/no-add-event-listener": "error",
			"svelte/no-at-const-tags": "error",
			"svelte/no-bind-value-on-checkable-inputs": "error",
			"svelte/no-conflicting-module-names": "error",
			"svelte/no-extra-reactive-curlies": "error",
			"svelte/no-ignored-unsubscribe": "error",
			"svelte/no-nested-style-tag": "error",
			"svelte/no-target-blank": "error",
			"svelte/no-top-level-browser-globals": "error",
			"svelte/prefer-attribute-interpolation": "error",
			"svelte/prefer-class-directive": "error",
			"svelte/prefer-const": "error",
			"svelte/prefer-derived-over-derived-by": "error",
			"svelte/prefer-destructured-store-props": "error",
			"svelte/prefer-style-directive": "error",
			"svelte/require-event-prefix": "error",
			"svelte/require-optimized-style-attribute": "error",
			"svelte/require-store-callbacks-use-set-param": "error",
			"svelte/require-stores-init": "error",
			"svelte/valid-compile": "error",
			"svelte/valid-style-parse": "error",
		},
	},
];
