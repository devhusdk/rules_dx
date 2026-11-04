import { test } from "node:test";
import { strict as assert } from "node:assert";
import { greet } from "./greet.js";

test("greets by name", () => {
	assert.equal(greet("ada"), "hello ada");
});
