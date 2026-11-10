import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import semver from "semver";
import { run } from "./cli.js";

const entry = fileURLToPath(import.meta.url);
const dataFile = fileURLToPath(
	new URL("../registry/packages.json", import.meta.url),
);
const cwd = process.env.BUILD_WORKING_DIRECTORY ?? process.cwd();

function where() {
	const require = createRequire(import.meta.url);
	const semverDir = resolve(require.resolve("semver/package.json"), "..");
	const pinned = require("semver/package.json").version;
	process.stdout.write(`entry ${entry}\n`);
	process.stdout.write(`data ${dataFile}\n`);
	process.stdout.write(`semver ${semverDir} ${pinned}\n`);
	process.stdout.write(`same ${semver === require("semver")}\n`);
	process.stdout.write(`argv ${JSON.stringify(process.argv.slice(2))}\n`);
}

const argv = process.argv.slice(2);
if (argv[0] === "where") {
	where();
} else {
	process.exitCode = run(
		argv,
		{
			out: (line) => process.stdout.write(`${line}\n`),
			err: (line) => process.stderr.write(`${line}\n`),
			read: (path) => readFileSync(path, "utf8"),
			resolve: (path) => resolve(cwd, path),
		},
		dataFile,
		process.env.REGISTRY_FILE,
	);
}
