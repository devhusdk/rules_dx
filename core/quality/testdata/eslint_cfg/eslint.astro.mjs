import eslintPluginAstro from "eslint-plugin-astro";

export default [
  ...eslintPluginAstro.configs.recommended,
  {
    files: ["**/*.astro"],
    languageOptions: {
      parserOptions: {
        parser: "@typescript-eslint/parser",
      },
    },
  },
  {
    files: ["**/*.astro", "**/*.astro/*.js", "**/*.astro/*.ts"],
    rules: {
      "no-unused-vars": "error",
    },
  },
];
