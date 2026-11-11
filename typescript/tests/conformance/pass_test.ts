import { greet } from "./greet.js";

declare function test(name: string, fn: () => void): void;
declare const expect: (actual: unknown) => {
	toBe(expected: unknown): void;
};

test("greets by name", () => {
	expect(greet("world")).toBe("hello world");
});
