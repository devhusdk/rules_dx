import { hello } from "./hello.js";

// @ts-expect-error node:test ships no local types here; node provides it at runtime.
import { test } from "node:test";

function equal(actual: unknown, expected: unknown): void {
	if (actual !== expected) {
		throw new Error(`want ${String(expected)} got ${String(actual)}`);
	}
}

test("greets by name", () => {
	equal(hello("world"), "hello world");
});
