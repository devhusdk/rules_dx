import prettier from "prettier";

export default [
  {
    files: ["**/*.vue"],
    rules: prettier.rules ?? {},
  },
];
