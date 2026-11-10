import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

// Compiles one .mdx file for jest with the @mdx-js/mdx the test links. The
// source map lets jest's coverage report the lines of the document, not of
// the generated code.
async function processAsync(src, filename, options) {
	const req = createRequire(path.join(options.config.rootDir, "package.json"));
	const { compile } = await import(
		pathToFileURL(req.resolve("@mdx-js/mdx")).href
	);
	const { SourceMapGenerator } = req("source-map");
	const file = await compile(
		{ value: src, path: filename },
		{ SourceMapGenerator, development: false, jsxImportSource: "preact" },
	);
	const map = file.map;
	map.sources = [filename];
	map.sourcesContent = [src];
	return { code: String(file.value), map };
}

export default { processAsync };
