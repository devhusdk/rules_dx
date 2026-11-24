"use strict";

function frameworkJestConfig(ext, transformPath) {
	return {
		transform: { ["\\." + ext + "$"]: transformPath },
		extensionsToTreatAsEsm: ["." + ext],
		moduleFileExtensions: ["js", "mjs", "cjs", "jsx", "ts", "tsx", "json", "node", ext],
		collectCoverageFrom: ["**/*.{cjs,js,jsx,mjs,ts,tsx," + ext + "}", "!**/node_modules/**"],
	};
}

module.exports = { frameworkJestConfig };
