import { createRequire } from "node:module";
import path from "node:path";
import { pathToFileURL } from "node:url";

// Compiles one .astro file for jest with the @astrojs/compiler-rs the test links.
// The source map lets jest's coverage report the lines of the component, not
// of the generated code.
async function processAsync(src, filename, options) {
	const req = createRequire(path.join(options.config.rootDir, "package.json"));
	const { transform } = await import(
		pathToFileURL(req.resolve("@astrojs/compiler-rs/async")).href
	);
	const result = await transform(src, {
		filename,
		sourcemap: "external",
		internalURL: "astro/compiler-runtime",
		resultScopedSlot: true,
		inlineComponentAssets: true,
		resolvePath: (specifier) => specifier,
	});
	const error = result.diagnostics.find((diag) => diag.severity === "error");
	if (error) {
		throw new Error(`${filename}: ${error.text}`);
	}
	const map = JSON.parse(
		typeof result.map === "string" ? result.map : JSON.stringify(result.map),
	);
	map.sources = [filename];
	map.sourcesContent = [src];
	return { code: result.code, map };
}

export default { processAsync };
