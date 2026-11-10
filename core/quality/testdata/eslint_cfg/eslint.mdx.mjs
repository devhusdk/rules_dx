import * as mdx from "eslint-plugin-mdx";

export default [
  {
    ...mdx.flat,
    files: ["**/*.mdx"],
    languageOptions: {
      ...mdx.flat.languageOptions,
      parserOptions: { ...mdx.flat.languageOptions.parserOptions, ignoreRemarkConfig: true },
    },
    rules: { ...mdx.flat.rules, "mdx/remark": "off", "no-unused-vars": "error" },
  },
];
