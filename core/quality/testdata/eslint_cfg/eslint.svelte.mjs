import svelte from "eslint-plugin-svelte";

export default [
  ...svelte.configs.recommended,
  {
    files: ["**/*.svelte"],
    languageOptions: {
      parserOptions: {
        parser: "@typescript-eslint/parser",
      },
    },
  },
  {
    files: ["**/*.svelte"],
    rules: {
      "no-unused-vars": "error",
    },
  },
];
