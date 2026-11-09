import { spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const here = path.dirname(fileURLToPath(import.meta.url));
const tool = path.join(process.env.RUNFILES_DIR ?? process.env.JS_BINARY__RUNFILES, process.env.VITE_TOOL);

test("svelte_build fails on a compile error and names the file", () => {
	const out = path.join(fs.mkdtempSync(path.join(os.tmpdir(), "build-")), "o");
	const child = spawnSync(
		process.execPath,
		[tool, "build", out, "vite.config.mjs"],
		{ cwd: path.join(here, "build_fail"), encoding: "utf8" },
	);
	expect(child.status).not.toBe(0);
	expect(child.stderr).toMatch(/src\/Broken\.svelte:\d+:\d+/);
	expect(fs.existsSync(path.join(out, "index.html"))).toBe(false);
});
