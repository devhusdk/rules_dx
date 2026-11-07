import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { latestStable, parseRegistry, schema } from "./registry.js";

const packaged = fileURLToPath(new URL("./packages.json", import.meta.url));

test("the packaged registry parses and every package resolves", () => {
	const entries = parseRegistry(readFileSync(packaged, "utf8"));
	assert.deepEqual(
		entries.map((entry) => entry.name),
		["core", "net", "edge"],
	);
	assert.deepEqual(
		entries.map((entry) => latestStable(entry)),
		["1.1.3", "2.1.1", undefined],
	);
});

test("the generated module carries the VERSION file", () => {
	assert.equal(
		schema(),
		Number(
			readFileSync(
				fileURLToPath(new URL("./VERSION", import.meta.url)),
				"utf8",
			),
		),
	);
});
