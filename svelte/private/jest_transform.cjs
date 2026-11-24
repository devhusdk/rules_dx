"use strict";

const { createRequire } = require("node:module");
const path = require("node:path");

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
