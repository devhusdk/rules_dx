import { spawnSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

process.env.NO_COLOR = "1";
process.env.FORCE_COLOR = "0";

function usage(message) {
	process.stderr.write(`svelte_tool: ${message}\n`);
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
		fs.mkdtempSync(path.join(os.tmpdir(), "svelte-tool-")),
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

function checkReport(report, tsconfig) {
	if (!report || !tsconfig)
		usage("check requires a report path and a tsconfig");
	const bin = resolvePath("svelte-check/bin/svelte-check");
	const child = spawnSync(
		process.execPath,
		[
			bin,
			"--workspace",
			process.cwd(),
			"--tsconfig",
			path.resolve(process.cwd(), tsconfig),
			"--output",
			"machine",
		],
		{ encoding: "utf8", env: process.env, maxBuffer: 64 * 1024 * 1024 },
	);
	const output = stripVTControlCharacters(
		`${child.stdout ?? ""}${child.stderr ?? ""}`,
	);
	// svelte-check exits 1 both on diagnostics and on a crash, so trust the
	// exit code only together with the COMPLETED summary line.
	const completed = /^\d+ COMPLETED \d+ FILES (\d+) ERRORS\b/m.exec(output);
	const crashed = /^\d+ FAILURE\b|svelte-check failed/m.test(output);
	const clean = child.status === 0 && completed && completed[1] === "0";
	const diagnostics =
		child.status === 1 && completed && completed[1] !== "0" && !crashed;
	if (child.error || crashed || !(clean || diagnostics)) {
		process.stderr.write(output);
		usage(
			`svelte-check did not produce a result (status ${child.status}, signal ${child.signal})`,
		);
	}
	// Drop the timestamps and the execroot START line to keep the report stable.
	const stable = output
		.split("\n")
		.filter((line) => !/^\d+ START\b/.test(line))
		.map((line) => line.replace(/^\d+ /, ""))
		.join("\n");
	fs.writeFileSync(
		path.resolve(process.cwd(), report),
		`exit_code: ${child.status}\n${stable}`,
	);
}

function verify(report) {
	if (!report) usage("verify requires a report path");
	const text = fs.readFileSync(report, "utf8");
	const match = /^exit_code: (\d+)\n/.exec(text);
	if (!match) usage(`${report} is not a svelte_check report`);
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
	case "verify":
		verify(rest[0]);
		break;
	default:
		usage(`unknown command '${command}'; expected build, check or verify`);
}
