"use strict";

const path = require("node:path");
const { frameworkJestConfig } = require("../../javascript/rules/framework_jest_config.cjs");

module.exports = {
	testEnvironment: "node",
	...frameworkJestConfig("astro", path.join(__dirname, "jest_transform.mjs")),
};
