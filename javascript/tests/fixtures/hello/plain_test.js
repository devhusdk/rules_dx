import { test } from "node:test";
import { strict as assert } from "node:assert";
import { hello } from "./hello.js";

test("greets by name", () => {
	assert.equal(hello("world"), "hello world");
});
