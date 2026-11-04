import { total } from "./app.js";

declare function test(name: string, fn: () => void): void;
declare const expect: (actual: unknown) => {
	toBe(expected: unknown): void;
};

test("totals small lists", () => {
	expect(total([1, 2, 3])).toBe(6);
});
