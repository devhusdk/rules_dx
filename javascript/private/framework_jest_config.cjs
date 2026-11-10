"use strict";

// Jest settings shared by the framework test rules (svelte_test, ...).
// `ext` is the component extension without the dot, `transformPath` the
// absolute path of the transform that compiles it.
function frameworkJestConfig(ext, transformPath) {
	return {
		transform: { ["\\." + ext + "$"]: transformPath },
		extensionsToTreatAsEsm: ["." + ext],
		moduleFileExtensions: ["js", "mjs", "cjs", "jsx", "ts", "tsx", "json", "node", ext],
		// Setting this replaces rules_jest's default, which lists only JS/TS extensions.
		collectCoverageFrom: ["**/*.{cjs,js,jsx,mjs,ts,tsx," + ext + "}", "!**/node_modules/**"],
	};
}

module.exports = { frameworkJestConfig };
