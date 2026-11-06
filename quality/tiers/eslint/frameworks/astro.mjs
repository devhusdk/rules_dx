import astro from "eslint-plugin-astro";

export default [
	...astro.configs.recommended,
	{
		files: ["**/*.astro"],
		languageOptions: { parserOptions: { parser: "@typescript-eslint/parser" } },
	},
];

export const strict = [
	{
		files: ["**/*.astro"],
		rules: {
			"astro/no-set-html-directive": "error",
			"astro/no-set-text-directive": "error",
			"astro/no-unsafe-inline-scripts": "error",
			"astro/no-unused-css-selector": "error",
			"astro/prefer-class-list-directive": "error",
			"astro/prefer-object-class-list": "error",
			"astro/prefer-split-class-list": "error",
			"astro/sort-attributes": "error",
		},
	},
];
