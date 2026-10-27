// Fails unless the wrapped test receives exactly `--marker <path>` and the
// file holds "dx args marker". See javascript/tests/fixtures/args for the
// path-resolution notes.
// @ts-expect-error node types are not installed; node provides them at runtime.
import { existsSync, readFileSync } from "node:fs";
// @ts-expect-error node types are not installed; node provides them at runtime.
import { isAbsolute, join } from "node:path";

declare const process: {
	argv: string[];
	env: Record<string, string | undefined>;
	cwd(): string;
	exit(code: number): never;
};

const args: string[] = process.argv.slice(2);
if (args.length !== 2 || args[0] !== "--marker") {
	console.error(`want --marker <path>, got ${JSON.stringify(args)}`);
	process.exit(1);
}
const rel: string = args[1];
const roots: string[] = [
	process.cwd(),
	process.env.RUNFILES_DIR ? join(process.env.RUNFILES_DIR, "_main") : "",
	process.env.JS_BINARY__RUNFILES
		? join(process.env.JS_BINARY__RUNFILES, "_main")
		: "",
].filter((r) => r !== "");
const candidates: string[] = isAbsolute(rel)
	? [rel]
	: roots.map((r) => join(r, rel));
const found = candidates.find((p) => existsSync(p));
if (found === undefined) {
	console.error(`cannot open ${rel}; tried ${JSON.stringify(candidates)}`);
	process.exit(1);
}
if (readFileSync(found, "utf8") !== "dx args marker\n") {
	console.error("unexpected marker contents");
	process.exit(1);
}
