import assert from "node:assert";
import { spawnSync } from "node:child_process";

const jsPath = process.argv.slice(2).find((f) => f.endsWith(".js"));
const result = spawnSync(process.execPath, [jsPath], {
	encoding: "utf8",
});
assert.strictEqual(result.status, 0, result.stderr);
assert.strictEqual(result.stdout.trim(), "hello from dart web");
