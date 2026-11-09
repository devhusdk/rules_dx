"use strict";

const { createRequire } = require("node:module");
const path = require("node:path");

// Compiles one .vue file for jest with the @vue/compiler-sfc the test links.
// The source map lets jest's coverage report the lines of the component, not
// of the generated code.
function process(src, filename, options) {
	const req = createRequire(path.join(options.config.rootDir, "package.json"));
	const { parse, compileScript } = req("@vue/compiler-sfc");
	const { descriptor, errors } = parse(src, { filename, sourceMap: true });
	if (errors.length > 0) {
		throw errors[0];
	}
	const script = compileScript(descriptor, {
		id: path.basename(filename),
		inlineTemplate: true,
		sourceMap: true,
	});
	const map = script.map;
	map.sources = [filename];
	map.sourcesContent = [src];
	return { code: script.content, map };
}

module.exports = { process };
