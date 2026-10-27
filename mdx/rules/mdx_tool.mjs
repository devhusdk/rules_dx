import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

const DIAGNOSTICS = 1;
const CRASH = 3;

process.env.NO_COLOR = "1";
process.env.FORCE_COLOR = "0";

function usage(message) {
	process.stderr.write(`mdx_tool: ${message}\n`);
	process.exit(2);
}

function crash(error) {
	process.stderr.write(`mdx_tool: crashed: ${error?.stack ?? error}\n`);
	process.exit(CRASH);
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
	const prefix = process.env.MDX_TOOL_PACKAGE ?? ".";
	const vite = await importPackage("vite");
	const cacheDir = path.join(
		fs.mkdtempSync(path.join(os.tmpdir(), "mdx-tool-")),
		"vite",
	);
	const outDir = path.resolve(root, out);
	try {
		await vite.build({
			root,
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

async function check(report, prefix, files) {
	if (!report || prefix === undefined)
		usage("check requires a report path, a package prefix and files");
	process.on("uncaughtException", crash);
	process.on("unhandledRejection", crash);
	const { compile } = await importPackage("@mdx-js/mdx");
	const lines = [];
	let failed = false;
	for (const file of files.filter((name) => name.endsWith(".mdx")).sort()) {
		const value = fs.readFileSync(file, "utf8");
		try {
			const compiled = await compile(
				{ path: file, value },
				{ outputFormat: "program" },
			);
			for (const message of compiled.messages) {
				if (message.fatal) failed = true;
				lines.push(formatMessage(prefix, file, message));
			}
		} catch (error) {
			if (error?.reason === undefined && error?.place === undefined) {
				crash(error);
			}
			failed = true;
			lines.push(formatMessage(prefix, file, error));
		}
	}
	const status = failed ? DIAGNOSTICS : 0;
	const body = lines.map((line) => `${line}\n`).join("");
	fs.writeFileSync(
		path.resolve(process.cwd(), report),
		`exit_code: ${status}\n${body}`,
	);
}

function verify(report) {
	if (!report) usage("verify requires a report path");
	const text = fs.readFileSync(report, "utf8");
	const match = /^exit_code: (\d+)\n/.exec(text);
	if (!match) usage(`${report} is not an mdx_check report`);
	if (match[1] !== "0") {
		process.stderr.write(text);
		process.exit(DIAGNOSTICS);
	}
}

const [command, ...rest] = process.argv.slice(2);
switch (command) {
	case "build":
		await build(rest[0], rest[1], rest[2]);
		break;
	case "check":
		await check(rest[0], rest[1], rest.slice(2));
		break;
	case "verify":
		verify(rest[0]);
		break;
	default:
		usage(`unknown command '${command}'; expected build, check or verify`);
}
