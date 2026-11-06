import vue from "eslint-plugin-vue";

export default [
  ...vue.configs["flat/recommended"],
  {
    files: ["**/*.vue"],
    languageOptions: { parserOptions: { parser: "@typescript-eslint/parser" } },
  },
];
