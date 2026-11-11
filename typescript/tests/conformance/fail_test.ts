import { greet } from "./greet.js";

declare function test(name: string, fn: () => void): void;
declare const expect: (actual: unknown) => {
	toBe(expected: unknown): void;
};

test("failing case", () => {
	expect(greet("world")).toBe("goodbye world");
});
