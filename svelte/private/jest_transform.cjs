"use strict";

const { createRequire } = require("node:module");
const path = require("node:path");

// Compiles one .svelte file for jest with the svelte the test links, so the
// compiler version always matches the runtime. The source map lets jest's
// coverage report the lines of the component, not of the generated code.
function process(src, filename, options) {
	const req = createRequire(path.join(options.config.rootDir, "package.json"));
	const { compile } = req("svelte/compiler");
	const { js } = compile(src, { filename, generate: "server" });
	const map = js.map;
	map.sources = [filename];
	map.sourcesContent = [src];
	return { code: js.code, map };
}

module.exports = { process };
