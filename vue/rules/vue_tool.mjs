import { spawnSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

const CRASH = 3;
const DIAGNOSTICS = /\berror TS\d+:/;

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

function crash(error) {
	process.stderr.write(`vue_tool: check crashed: ${error?.stack ?? error}\n`);
	process.exit(CRASH);
}

function checkRaw(tsconfig) {
	if (!tsconfig) usage("check-raw requires a tsconfig");
	const bin = resolvePath("vue-tsc/bin/vue-tsc.js");
	process.on("uncaughtException", crash);
	process.on("unhandledRejection", crash);
	process.argv = [
		process.argv[0],
		bin,
		"--noEmit",
		"--pretty",
		"false",
		"-p",
		path.resolve(process.cwd(), tsconfig),
	];
	try {
		createRequire(bin)(bin);
	} catch (error) {
		crash(error);
	}
}

function checkReport(report, tsconfig) {
	if (!report || !tsconfig)
		usage("check requires a report path and a tsconfig");
	const child = spawnSync(
		process.execPath,
		[process.argv[1], "check-raw", tsconfig],
		{ encoding: "utf8", env: process.env, maxBuffer: 64 * 1024 * 1024 },
	);
	const output = stripVTControlCharacters(
		`${child.stdout ?? ""}${child.stderr ?? ""}`,
	);
	const diagnostics =
		(child.status === 1 || child.status === 2) && DIAGNOSTICS.test(output);
	if (child.error || (child.status !== 0 && !diagnostics)) {
		process.stderr.write(output);
		usage(
			`vue-tsc did not produce a result (status ${child.status}, signal ${child.signal})`,
		);
	}
	fs.writeFileSync(
		path.resolve(process.cwd(), report),
		`exit_code: ${child.status}\n${output}`,
	);
}

function verify(report) {
	if (!report) usage("verify requires a report path");
	const text = fs.readFileSync(report, "utf8");
	const match = /^exit_code: (\d+)\n/.exec(text);
	if (!match) usage(`${report} is not a vue_check report`);
	if (match[1] !== "0") {
		process.stderr.write(text);
		process.exit(1);
	}
}

const [command, ...rest] = process.argv.slice(2);
switch (command) {
	case "build":
		await build(rest[0], rest[1], rest[2]);
		break;
	case "check":
		checkReport(rest[0], rest[1]);
		break;
	case "check-raw":
		checkRaw(rest[0]);
		break;
	case "verify":
		verify(rest[0]);
		break;
	default:
		usage(`unknown command '${command}'; expected build, check or verify`);
}
