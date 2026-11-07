// Seed Jest test over the tsc-compiled output via typescript_test.
// Static-import coverage of TypeScript sources uses the tsc emit (the
// transform slice for direct TS execution is future work); the test
// imports the compiled `./hello.js` from the runfiles tree via
// `deps = [":hello_lib"]`, mirroring
// //javascript/tests/fixtures/hello:hello_test.
import { hello } from "./hello.js";

declare function test(name: string, fn: () => void): void;
declare const expect: (actual: unknown) => {
	toBe(expected: unknown): void;
};

test("greets by name", () => {
	expect(hello("world")).toBe("hello world");
});
