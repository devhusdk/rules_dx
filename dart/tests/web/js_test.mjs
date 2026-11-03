import assert from "node:assert";
import { spawnSync } from "node:child_process";

const result = spawnSync(process.execPath, [process.argv[2]], {
	encoding: "utf8",
});
assert.strictEqual(result.status, 0, result.stderr);
assert.strictEqual(result.stdout.trim(), "hello from dart web");
