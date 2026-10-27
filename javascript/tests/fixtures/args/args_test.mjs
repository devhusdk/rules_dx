// Fails unless the wrapped test receives exactly `--marker <path>` and the
// file holds "dx args marker".
//
// rules_js js_binary may chdir away from the runfiles root, so the path is
// resolved against cwd first, then against the runfiles root.
import { existsSync, readFileSync } from "node:fs";
import { join, isAbsolute } from "node:path";

const args = process.argv.slice(2);
if (args.length !== 2 || args[0] !== "--marker") {
	console.error(`want --marker <path>, got ${JSON.stringify(args)}`);
	process.exit(1);
}
const rel = args[1];
const roots = [
	process.cwd(),
	process.env.RUNFILES_DIR && join(process.env.RUNFILES_DIR, "_main"),
	process.env.JS_BINARY__RUNFILES &&
		join(process.env.JS_BINARY__RUNFILES, "_main"),
].filter(Boolean);
const candidates = isAbsolute(rel) ? [rel] : roots.map((r) => join(r, rel));
const found = candidates.find((p) => existsSync(p));
if (!found) {
	console.error(`cannot open ${rel}; tried ${JSON.stringify(candidates)}`);
	process.exit(1);
}
if (readFileSync(found, "utf8") !== "dx args marker\n") {
	console.error("unexpected marker contents");
	process.exit(1);
}
