import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

process.env.NO_COLOR = "1";
process.env.FORCE_COLOR = "0";

function usage(message) {
	process.stderr.write(`vue_tool: ${message}\n`);
	process.exit(2);
}

function resolvePath(specifier) {
	const require = createRequire(path.join(process.cwd(), "package.json"));
	try {
		return require.resolve(specifier);
	} catch (error) {
		usage(
			`cannot resolve '${specifier}' from ${process.cwd()}; add it to node_modules (${error.code})`,
		);
	}
}

function nonEmptyDir(dir) {
	return fs.existsSync(dir) && fs.readdirSync(dir).length > 0;
}

async function viteBuild(vite, root, config, cacheDir, build) {
	await vite.build({
		root,
		configFile: path.resolve(root, config),
		cacheDir,
		logLevel: "warn",
		build: { emptyOutDir: true, ...build },
	});
}

async function build(out, config, ssrEntry) {
	if (!out || !config) usage("build requires an output directory and a config");
	const root = process.cwd();
	const vite = await import(pathToFileURL(resolvePath("vite")).href);
	const cacheDir = path.join(
		fs.mkdtempSync(path.join(os.tmpdir(), "vue-tool-")),
		"vite",
	);
	const outDir = path.resolve(root, out);
	await viteBuild(vite, root, config, cacheDir, {
		outDir,
		...(ssrEntry ? { ssr: ssrEntry, copyPublicDir: false } : {}),
	});
	if (ssrEntry && !nonEmptyDir(outDir)) {
		usage(`SSR build produced an empty ${out}`);
	}
	if (!ssrEntry && !fs.existsSync(path.join(outDir, "index.html"))) {
		usage(`build produced no ${out}/index.html`);
	}
}

const [command, ...rest] = process.argv.slice(2);
switch (command) {
	case "build":
		await build(rest[0], rest[1], rest[2]);
		break;
	default:
		usage(`unknown command '${command}'; expected build`);
}
