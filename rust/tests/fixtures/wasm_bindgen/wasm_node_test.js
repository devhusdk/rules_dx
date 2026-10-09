import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";
import {
	add,
	add_via_snippet,
	greet,
	initSync,
} from "./wasm_hello_web/wasm_hello_web.js";

const bytes = readFileSync(
	new URL("./wasm_hello_web/wasm_hello_web_bg.wasm", import.meta.url),
);
initSync({ module: bytes });

test("calls greet through the generated bindings", () => {
	assert.equal(greet("dx"), "Hello from wasm, dx!");
});

test("calls add through the generated bindings", () => {
	assert.equal(add(40, 2), 42);
});

test("calls into the snippet through the generated bindings", () => {
	assert.equal(add_via_snippet(40, 2), 42);
});
