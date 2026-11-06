import astro from "eslint-plugin-astro";

export default [
  ...astro.configs.recommended,
  {
    files: ["**/*.astro"],
    languageOptions: { parserOptions: { parser: "@typescript-eslint/parser" } },
  },
];
