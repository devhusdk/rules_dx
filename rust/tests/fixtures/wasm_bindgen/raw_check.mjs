import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFile } from "node:fs/promises";
import path from "node:path";

function dataPath(value) {
	if (path.isAbsolute(value)) {
		return value;
	}
	const srcdir = process.env.TEST_SRCDIR ?? "";
	if (value.startsWith("../")) {
		return path.join(srcdir, value);
	}
	return path.join(srcdir, process.env.TEST_WORKSPACE ?? "", value);
}

test("adds through raw wasm without bindings", async () => {
	const wasmPath = process.env.DX_WASM_RAW ?? process.argv[2] ?? "";
	assert.notEqual(wasmPath, "");
	const bytes = await readFile(dataPath(wasmPath));
	const { instance } = await WebAssembly.instantiate(bytes);
	assert.equal(instance.exports.dx_wasm_add(40, 2), 42);
});
