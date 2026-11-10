"use strict";

const path = require("node:path");
const { frameworkJestConfig } = require("../../javascript/private/framework_jest_config.cjs");

module.exports = {
	testEnvironment: "node",
	...frameworkJestConfig("svelte", path.join(__dirname, "jest_transform.cjs")),
};
