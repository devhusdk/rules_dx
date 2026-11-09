import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

const DIAGNOSTICS = 1;

process.env.NO_COLOR = "1";
process.env.FORCE_COLOR = "0";

function usage(message) {
	process.stderr.write(`vite_tool: ${message}\n`);
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

async function importPackage(specifier) {
	return import(pathToFileURL(resolvePath(specifier)).href);
}

function nonEmptyDir(dir) {
	return fs.existsSync(dir) && fs.readdirSync(dir).length > 0;
}

function workspacePath(prefix, file) {
	const relative = path.relative(process.cwd(), path.resolve(file));
	const local = relative.startsWith("..") ? file : relative;
	return path.posix.join(prefix, local.split(path.sep).join("/"));
}

function formatMessage(prefix, fallbackFile, message) {
	const loc = message.loc ?? {};
	const place = message.place?.start ?? message.place ?? {};
	const reason = stripVTControlCharacters(
		String(message.reason ?? message.message ?? message),
	)
		.split("\n")[0]
		.trim();
	const quoted = /\((\d+):(\d+)(?:-\d+:\d+)?\)$/.exec(reason) ?? [];
	const file = loc.file || message.file || message.id || fallbackFile;
	const line = message.line ?? loc.line ?? place.line ?? quoted[1];
	const column = message.column ?? loc.column ?? place.column ?? quoted[2];
	const where = [file ? workspacePath(prefix, file) : prefix];
	if (line) where.push(line, column ?? 1);
	const rule = [message.source, message.ruleId].filter(Boolean).join(":");
	return `${where.join(":")}: ${reason}${rule ? ` [${rule}]` : ""}`;
}

function buildMessages(prefix, error) {
	const causes = Array.isArray(error?.errors) ? error.errors : [error];
	return causes.map((cause) => formatMessage(prefix, undefined, cause));
}

async function build(out, config, ssrEntry) {
	if (!out || !config) usage("build requires an output directory and a config");
	const root = process.cwd();
	const prefix = process.env.VITE_TOOL_PACKAGE ?? ".";
	const vite = await importPackage("vite");
	const cacheDir = path.join(
		fs.mkdtempSync(path.join(os.tmpdir(), "vite-tool-")),
		"vite",
	);
	const outDir = path.resolve(root, out);
	// Rolldown canonicalizes the HTML entry through the real execroot inside a
	// sandbox, which makes Vite derive an escaping asset name from it.
	const entry = path.join(root, "index.html");
	const keepEntryPath = {
		name: "vite-tool:keep-entry-path",
		enforce: "pre",
		resolveId: (id) => (path.resolve(id) === entry ? entry : null),
	};
	try {
		await vite.build({
			root,
			plugins: process.env.VITE_TOOL_KEEP_ENTRY_PATH ? [keepEntryPath] : [],
			configFile: path.resolve(root, config),
			cacheDir,
			logLevel: "warn",
			build: {
				emptyOutDir: true,
				outDir,
				...(ssrEntry ? { ssr: ssrEntry, copyPublicDir: false } : {}),
			},
		});
	} catch (error) {
		for (const line of buildMessages(prefix, error)) {
			process.stderr.write(`${line}\n`);
		}
		process.exit(DIAGNOSTICS);
	}
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
