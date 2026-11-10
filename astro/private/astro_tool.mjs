import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

process.env.ASTRO_TELEMETRY_DISABLED = "1";
process.env.NO_COLOR = "1";
process.env.FORCE_COLOR = "0";

function usage(message) {
	process.stderr.write(`astro_tool: ${message}\n`);
	process.exit(2);
}

function resolveFromCwd(specifier) {
	const require = createRequire(path.join(process.cwd(), "package.json"));
	try {
		return pathToFileURL(require.resolve(specifier)).href;
	} catch (error) {
		usage(
			`cannot resolve '${specifier}' from ${process.cwd()}; add it to node_modules (${error.code})`,
		);
	}
}

function scratchConfig(root, config) {
	if (!config) usage("a config file is required");
	const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "astro-tool-"));
	return {
		root,
		configFile: config,
		cacheDir: path.join(scratch, "astro"),
		logLevel: "warn",
		telemetry: false,
		vite: { cacheDir: path.join(scratch, "vite") },
	};
}

async function build(out, config, ssrEntry) {
	if (!out) usage("build requires an output directory");
	const root = process.cwd();
	const astro = await import(resolveFromCwd("astro"));
	await astro.build({
		...scratchConfig(root, config),
		outDir: path.resolve(root, out),
	});
	if (ssrEntry) {
		if (fs.readdirSync(path.resolve(root, out)).length === 0) {
			usage(`SSR build produced an empty ${out}`);
		}
	} else if (!fs.existsSync(path.resolve(root, out, "index.html"))) {
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
