// Match the one-source `*_test.js` test naming
// (the generation contract recognizes `_test` suffix, not `.test.` infix).
module.exports = {
  testEnvironment: "node",
  testMatch: ["**/*_test.js"],
};
