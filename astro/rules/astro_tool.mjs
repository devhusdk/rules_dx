import { spawnSync } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { stripVTControlCharacters } from "node:util";

const CRASH = 3;

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

function scratchConfig(root) {
	const scratch = fs.mkdtempSync(path.join(os.tmpdir(), "astro-tool-"));
	return {
		root,
		cacheDir: path.join(scratch, "astro"),
		logLevel: "warn",
		telemetry: false,
		vite: { cacheDir: path.join(scratch, "vite") },
	};
}

async function build(out) {
	if (!out) usage("build requires an output directory");
	const root = process.cwd();
	const astro = await import(resolveFromCwd("astro"));
	await astro.build({
		...scratchConfig(root),
		outDir: path.resolve(root, out),
	});
	if (!fs.existsSync(path.resolve(root, out, "index.html"))) {
		usage(`build produced no ${out}/index.html`);
	}
}

async function checkRaw(tsconfig) {
	const root = process.cwd();
	try {
		const astro = await import(resolveFromCwd("astro"));
		await astro.sync(scratchConfig(root));
		const checker = await import(resolveFromCwd("@astrojs/check"));
		const failed = await checker.check({
			root,
			tsconfig: tsconfig ? path.resolve(root, tsconfig) : undefined,
			minimumSeverity: "hint",
			minimumFailingSeverity: "error",
		});
		if (typeof failed !== "boolean") {
			process.stderr.write("astro_tool: @astrojs/check returned no result\n");
			process.exit(CRASH);
		}
		process.exit(failed ? 1 : 0);
	} catch (error) {
		process.stderr.write(
			`astro_tool: check crashed: ${error?.stack ?? error}\n`,
		);
		process.exit(CRASH);
	}
}

function checkReport(report, tsconfig) {
	if (!report) usage("check requires a report path");
	const child = spawnSync(
		process.execPath,
		[process.argv[1], "check-raw", tsconfig ?? ""],
		{ encoding: "utf8", env: process.env, maxBuffer: 64 * 1024 * 1024 },
	);
	const output = stripVTControlCharacters(
		`${child.stdout ?? ""}${child.stderr ?? ""}`,
	);
	if (child.error || (child.status !== 0 && child.status !== 1)) {
		process.stderr.write(output);
		usage(
			`astro check did not produce a result (status ${child.status}, signal ${child.signal})`,
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
	if (!match) usage(`${report} is not an astro_check report`);
	if (match[1] !== "0") {
		process.stderr.write(text);
		process.exit(1);
	}
}

const [command, ...rest] = process.argv.slice(2);
switch (command) {
	case "build":
		await build(rest[0]);
		break;
	case "check":
		checkReport(rest[0], rest[1]);
		break;
	case "check-raw":
		await checkRaw(rest[0]);
		break;
	case "verify":
		verify(rest[0]);
		break;
	default:
		usage(`unknown command '${command}'; expected build, check or verify`);
}
