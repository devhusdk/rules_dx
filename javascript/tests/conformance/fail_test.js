import { greet } from "./greet.js";

test("failing case", () => {
	expect(greet("world")).toBe("goodbye world");
});
