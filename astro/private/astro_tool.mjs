import { createHash } from "node:crypto";
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

// Astro hashes the absolute path of each island's component into the island's
// uid, so the app is copied to a path with neither the sandbox directory nor a
// random suffix in it: <tmp>/astro-src-<hash>/<bindir>/<package>. Every
// node_modules on the way up from the app is linked at the same relative place,
// so packages resolve from the copy as they do from the app.
function materialize(root, out) {
	const bindir = process.env.BAZEL_BINDIR;
	if (!bindir) usage("BAZEL_BINDIR is not set; run astro_tool through js_run_binary");
	const relative = path.join(bindir, process.env.VITE_TOOL_PACKAGE ?? "");
	const execroot = root.slice(0, root.length - relative.length);
	if (path.join(execroot, relative) !== root) usage(`${root} does not end in ${relative}`);
	const hash = createHash("sha256").update(relative).digest("hex").slice(0, 16);
	const base = path.join(os.tmpdir(), `astro-src-${hash}`);
	fs.rmSync(base, { recursive: true, force: true });
	const copy = path.join(base, relative);
	fs.mkdirSync(copy, { recursive: true });
	const skip = new Set(["node_modules", out]);
	for (const entry of fs.readdirSync(root)) {
		if (skip.has(entry)) continue;
		fs.cpSync(path.join(root, entry), path.join(copy, entry), {
			recursive: true,
			dereference: true,
			filter: (source) => path.basename(source) !== "node_modules",
		});
	}
	for (let dir = root; ; dir = path.dirname(dir)) {
		const modules = path.join(dir, "node_modules");
		if (fs.existsSync(modules)) {
			const link = path.join(base, path.relative(execroot, dir), "node_modules");
			fs.mkdirSync(path.dirname(link), { recursive: true });
			fs.symlinkSync(modules, link, "junction");
		}
		if (path.relative(execroot, dir) === "") break;
	}
	return { base, copy };
}

async function build(out, config, ssrEntry) {
	if (!out) usage("build requires an output directory");
	const root = process.cwd();
	const astro = await import(resolveFromCwd("astro"));
	const { base, copy } = materialize(root, out);
	try {
		await astro.build({
			...scratchConfig(copy, config),
			outDir: path.resolve(root, out),
		});
	} finally {
		fs.rmSync(base, { recursive: true, force: true });
	}
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
