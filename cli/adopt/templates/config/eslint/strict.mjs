import standard from "./standard.mjs";

export default [
  ...standard,
  {
    rules: {
      "no-console": "error",
      "no-shadow": "error",
      "no-param-reassign": "error",
      complexity: ["error", 15],
    },
  },
];
