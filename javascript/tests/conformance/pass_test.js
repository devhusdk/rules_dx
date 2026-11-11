import { greet } from "./greet.js";

test("greets by name", () => {
	expect(greet("world")).toBe("hello world");
});
