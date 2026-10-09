import { total } from "./app.js";

// @ts-expect-error node:test ships no local types here; node provides it at runtime.
import { test } from "node:test";

function equal(actual: unknown, expected: unknown): void {
	if (actual !== expected) {
		throw new Error(`want ${String(expected)} got ${String(actual)}`);
	}
}

test("totals small lists", () => {
	equal(total([1, 2, 3]), 6);
});
